import { createHash, randomBytes } from "node:crypto";
import { readFileSync } from "node:fs";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import type { Duplex } from "node:stream";
import { generatedFixture, lapRecord, type FixtureEntry } from "./fixture.js";
import { SDK_VERSION, SCHEMA_VERSION } from "./types.js";
import type { GameEvent, LapRecord, Snapshot } from "./types.js";

export interface MockServerOptions {
  port?: number;
  token?: string;
  online?: boolean;
  speed?: number;
  loop?: boolean;
  fixture?: string;
}

export interface MockServer {
  readonly port: number;
  readonly token: string;
  readonly url: string;
  close(): Promise<void>;
}

const tools = [
  { name: "get_game_state", description: "Get the current game snapshot.", inputSchema: { type: "object", properties: {} } },
  { name: "get_last_lap", description: "Get the most recent local lap.", inputSchema: { type: "object", properties: {} } },
  { name: "get_session_laps", description: "Get local laps in this session.", inputSchema: { type: "object", properties: { limit: { type: "number" } } } },
  { name: "get_standings", description: "Get the public timing screen.", inputSchema: { type: "object", properties: {} } },
  { name: "get_recent_events", description: "Get recent game events.", inputSchema: { type: "object", properties: { limit: { type: "number" }, type: { type: "string" } } } },
] as const;

function json(res: ServerResponse, status: number, body: unknown) {
  const data = JSON.stringify(body);
  res.writeHead(status, { "content-type": "application/json", "content-length": Buffer.byteLength(data) });
  res.end(data);
}

function frame(text: string): Buffer {
  const body = Buffer.from(text);
  if (body.length < 126) return Buffer.concat([Buffer.from([0x81, body.length]), body]);
  if (body.length <= 0xffff) {
    const head = Buffer.alloc(4); head[0] = 0x81; head[1] = 126; head.writeUInt16BE(body.length, 2);
    return Buffer.concat([head, body]);
  }
  const head = Buffer.alloc(10); head[0] = 0x81; head[1] = 127; head.writeBigUInt64BE(BigInt(body.length), 2);
  return Buffer.concat([head, body]);
}

function control(opcode: number, body: Uint8Array = new Uint8Array()): Buffer {
  return Buffer.concat([Buffer.from([0x80 | opcode, body.length]), Buffer.from(body)]);
}

function fixtureFile(path: string): FixtureEntry[] {
  return readFileSync(path, "utf8").split(/\r?\n/).filter(Boolean).map((line, i) => {
    const value = JSON.parse(line) as FixtureEntry;
    if ((value.kind !== "state" && value.kind !== "event") || !Number.isFinite(value.offsetMs) || !value.data) throw new Error(`invalid fixture line ${i + 1}`);
    return value;
  }).sort((a, b) => a.offsetMs - b.offsetMs);
}

function fairState(input: Snapshot, online: boolean): Snapshot {
  const state = structuredClone(input);
  state.online = online;
  if (state.session) state.session.server = online ? (state.session.server ?? "Example Server") : null;
  if (online) for (const rider of state.riders) if (!rider.local) rider.track = null;
  return state;
}

/** Start a local, read-only Game SDK server backed by a synthetic or recorded fixture. */
export async function startMockServer(opts: MockServerOptions = {}): Promise<MockServer> {
  const host = "127.0.0.1";
  const token = opts.token ?? randomBytes(12).toString("hex");
  const online = opts.online ?? false;
  const speed = Number.isFinite(opts.speed) && (opts.speed ?? 0) > 0 ? opts.speed! : 1;
  const loop = opts.loop ?? true;
  const entries = opts.fixture ? fixtureFile(opts.fixture) : generatedFixture(online);
  if (!entries.length || !entries.some((e) => e.kind === "state")) throw new Error("fixture needs at least one state line");
  const duration = Math.max(1, entries.at(-1)!.offsetMs + 500);
  let current = fairState((entries.find((e) => e.kind === "state")! as Extract<FixtureEntry, { kind: "state" }>).data, online);
  let eventLog: GameEvent[] = [];
  let laps: LapRecord[] = [];
  let nextSeq = 0;
  let cursor = -1;
  let cycle = 0;
  const started = Date.now();

  const advance = () => {
    const raw = (Date.now() - started) * speed;
    const wantedCycle = Math.floor(raw / duration);
    const targetCycle = loop ? wantedCycle : 0;
    if (targetCycle !== cycle) { cycle = targetCycle; cursor = -1; laps = []; }
    const elapsed = loop ? raw % duration : Math.min(raw, duration);
    while (cursor + 1 < entries.length && entries[cursor + 1].offsetMs <= elapsed) {
      const entry = entries[++cursor];
      if (entry.kind === "state") {
        current = fairState(entry.data, online);
        current.at = Date.now();
      } else {
        const source = entry.data;
        if (online && (source.type === "crash" || source.type === "recover") && !source.local) continue;
        const event = { ...source, seq: ++nextSeq, at: Date.now() } as GameEvent;
        if (event.type === "event_start" || event.type === "session_start") laps = [];
        if (event.type === "lap") { const record = lapRecord(event); if (record) laps.push(record); }
        eventLog.push(event);
        if (eventLog.length > 500) eventLog.shift();
      }
    }
  };
  const clock = setInterval(advance, 10);

  let activeSockets = 0;
  const sockets = new Set<Duplex>();
  let actualPort = 0;
  const validHost = (req: IncomingMessage) => req.headers.host === `127.0.0.1:${actualPort}` || req.headers.host === `localhost:${actualPort}`;
  const authorized = (req: IncomingMessage, url: URL) => req.headers.authorization === `Bearer ${token}` || url.searchParams.get("token") === token;
  const server = createServer(async (req, res) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    if (!validHost(req)) return json(res, 403, { error: "forbidden host" });
    if (url.pathname.startsWith("/v1/") && req.method !== "GET") return json(res, 405, { error: "method not allowed" });
    if (url.pathname === "/v1/health") return json(res, 200, { ok: true, schema: SCHEMA_VERSION, sdk: SDK_VERSION, app: "mock" });
    if ((url.pathname.startsWith("/v1/") || url.pathname === "/mcp") && !authorized(req, url)) return json(res, 401, { error: "missing or wrong token" });
    advance();
    if (url.pathname === "/v1/hello" && req.method === "GET") return json(res, 200, hello());
    if (url.pathname === "/v1/state" && req.method === "GET") return json(res, 200, current);
    if (url.pathname === "/v1/events" && req.method === "GET") {
      const since = Number(url.searchParams.get("since") ?? 0);
      return json(res, 200, { events: eventLog.filter((e) => e.seq > since), last: eventLog.at(-1)?.seq ?? 0 });
    }
    if (url.pathname === "/v1/laps" && req.method === "GET") return json(res, 200, { laps });
    if (url.pathname === "/mcp" && req.method === "POST") {
      let body = ""; for await (const part of req) body += part;
      let call: { jsonrpc?: string; id?: unknown; method?: string; params?: { name?: string; arguments?: { limit?: number; type?: string } } };
      try { call = JSON.parse(body); } catch { return json(res, 400, { error: "invalid JSON" }); }
      const result = mcp(call.method, call.params);
      return json(res, 200, result.error
        ? { jsonrpc: "2.0", id: call.id ?? null, error: { code: -32601, message: result.error } }
        : { jsonrpc: "2.0", id: call.id ?? null, result: result.value });
    }
    return json(res, 404, { error: "not found" });
  });

  const hello = () => ({ schema: SCHEMA_VERSION, sdk: SDK_VERSION, app: "mock", capabilities: ["state", "events", "laps", "stream", "mcp"], deprecated: [] });
  const mcp = (method?: string, params?: { name?: string; arguments?: { limit?: number; type?: string } }): { value?: unknown; error?: string } => {
    if (method === "initialize") return { value: { protocolVersion: "2025-03-26", capabilities: { tools: {} }, serverInfo: { name: "mxb-game-mock", version: SDK_VERSION } } };
    if (method === "tools/list") return { value: { tools } };
    if (method !== "tools/call") return { error: "method not found" };
    const args = params?.arguments ?? {};
    let value: unknown;
    if (params?.name === "get_game_state") value = current;
    else if (params?.name === "get_last_lap") value = laps.at(-1) ?? null;
    else if (params?.name === "get_session_laps") value = laps.slice(-Math.max(0, args.limit ?? laps.length));
    else if (params?.name === "get_standings") value = { session: current.session, riders: current.riders };
    else if (params?.name === "get_recent_events") value = eventLog.filter((e) => !args.type || e.type === args.type).slice(-Math.max(0, args.limit ?? 20));
    else return { error: "tool not found" };
    return { value: { content: [{ type: "text", text: JSON.stringify(value) }] } };
  };

  server.on("upgrade", (req, socket: Duplex) => {
    const url = new URL(req.url ?? "/", `http://${req.headers.host ?? "localhost"}`);
    if (!validHost(req)) return rejectUpgrade(socket, 403);
    if (url.pathname !== "/v1/stream") return rejectUpgrade(socket, 404);
    if (!authorized(req, url)) return rejectUpgrade(socket, 401);
    if (activeSockets >= 16) return rejectUpgrade(socket, 503);
    const key = req.headers["sec-websocket-key"];
    if (typeof key !== "string") return rejectUpgrade(socket, 400);
    const accept = createHash("sha1").update(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").digest("base64");
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    activeSockets++;
    sockets.add(socket);
    const rate = Math.max(1, Math.min(60, Math.round(Number(url.searchParams.get("rate") ?? 20) || 20)));
    const topicText = url.searchParams.get("topics") ?? "state,events";
    const state = topicText.split(",").includes("state");
    const events = topicText.split(",").includes("events");
    let last = eventLog.at(-1)?.seq ?? 0;
    let ended = false;
    const statePeriod = 1000 / rate;
    let nextState = Date.now() + statePeriod;
    const send = (value: unknown) => { if (!ended && socket.writable) socket.write(frame(JSON.stringify(value))); };
    send({ type: "hello", ...hello(), rate });
    if (state) send({ type: "state", data: current });
    const timer = setInterval(() => {
      advance();
      if (state && Date.now() >= nextState) { send({ type: "state", data: current }); nextState = Date.now() + statePeriod; }
      if (events) for (const event of eventLog) if (event.seq > last) { send({ type: "event", data: event }); last = event.seq; }
    }, 10);
    const finish = () => { if (ended) return; ended = true; clearInterval(timer); activeSockets--; sockets.delete(socket); if (!socket.destroyed) socket.destroy(); };
    socket.on("data", (chunk: Buffer) => {
      let offset = 0;
      while (offset + 2 <= chunk.length) {
        const opcode = chunk[offset] & 0x0f; let length = chunk[offset + 1] & 0x7f; let head = 2;
        if (length === 126 && offset + 4 <= chunk.length) { length = chunk.readUInt16BE(offset + 2); head = 4; }
        else if (length === 127 && offset + 10 <= chunk.length) { length = Number(chunk.readBigUInt64BE(offset + 2)); head = 10; }
        const masked = (chunk[offset + 1] & 0x80) !== 0; const maskSize = masked ? 4 : 0;
        if (offset + head + maskSize + length > chunk.length) break;
        const body = Buffer.from(chunk.subarray(offset + head + maskSize, offset + head + maskSize + length));
        if (masked) {
          const mask = chunk.subarray(offset + head, offset + head + 4);
          for (let i = 0; i < body.length; i++) body[i] ^= mask[i % 4];
        }
        if (opcode === 8) { socket.write(control(8)); finish(); return; }
        if (opcode === 9) socket.write(control(10, body));
        offset += head + maskSize + length;
      }
    });
    socket.on("close", finish); socket.on("error", finish);
  });

  await new Promise<void>((resolve, reject) => { server.once("error", reject); server.listen(opts.port ?? 47_820, host, resolve); });
  actualPort = (server.address() as { port: number }).port;
  return { port: actualPort, token, url: `http://${host}:${actualPort}`, close: () => new Promise((resolve, reject) => {
    clearInterval(clock);
    for (const socket of sockets) socket.destroy();
    server.close((error) => error ? reject(error) : resolve());
  }) };
}

function rejectUpgrade(socket: Duplex, status: number) {
  socket.end(`HTTP/1.1 ${status} Error\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n${JSON.stringify({ error: "upgrade rejected" })}`);
}
