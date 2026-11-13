// The client against a stand-in for MXB App's endpoint: auth, the REST calls, and the stream's
// routing of frames and typed events.
import { afterAll, expect, test } from "bun:test";
import { GameSdk, GameSdkError, formatLap, type GameEvent, type Snapshot } from "../src/index";

const TOKEN = "t".repeat(48);
const INCOMPATIBLE_TOKEN = "x".repeat(48);
const snapshot = {
  schema: 1,
  frame: 7,
  at: 1,
  source: "replay",
  online: true,
  session: null,
  local: null,
  riders: [],
  frostmod: null,
} satisfies Snapshot;
const lap = { seq: 3, at: 2, type: "lap", local: true, raceNum: 12, lap: 4, timeMs: 95123, valid: true, best: false } as GameEvent;
const crash = { seq: 4, at: 3, type: "crash", local: true, raceNum: 12 } as GameEvent;
let streamQuery = "";

const server = Bun.serve<{ url: string }>({
  port: 0,
  fetch(req, srv) {
    const url = new URL(req.url);
    const supplied = req.headers.get("authorization")?.replace("Bearer ", "") ?? url.searchParams.get("token");
    const auth = supplied === TOKEN || supplied === INCOMPATIBLE_TOKEN;
    if (url.pathname === "/v1/health") return Response.json({ ok: true });
    if (!auth) return Response.json({ error: "missing or wrong token" }, { status: 401 });
    if (url.pathname === "/v1/hello")
      return Response.json({ schema: supplied === INCOMPATIBLE_TOKEN ? 2 : 1, sdk: "1.1.0", app: "test", capabilities: ["state"], frostmod: null, deprecated: [] });
    if (url.pathname === "/v1/state") return Response.json(snapshot);
    if (url.pathname === "/v1/actions" && req.method === "POST")
      return req.json().then((b: { action?: string }) =>
        b.action === "refresh" ? Response.json({ id: "a1-x", action: "refresh", state: "sent" }, { status: 202 })
          : Response.json({ error: "unknown action" }, { status: 409 }));
    if (url.pathname === "/v1/actions")
      return Response.json({ id: url.searchParams.get("id"), action: "refresh", state: "done", detail: "", lists: ["tracks"], at: 1 });
    if (url.pathname === "/v1/stream") {
      streamQuery = url.search;
      return srv.upgrade(req, { data: { url: req.url } }) ? undefined : new Response("no", { status: 400 });
    }
    return Response.json({ error: "not found" }, { status: 404 });
  },
  websocket: {
    open(ws) {
      const incompatible = new URL(ws.data.url).searchParams.get("token") === INCOMPATIBLE_TOKEN;
      ws.send(JSON.stringify({ type: "hello", schema: incompatible ? 2 : 1, sdk: "1.1.0", app: "test", capabilities: [], frostmod: null, deprecated: [], rate: 20 }));
      ws.send(JSON.stringify({ type: "state", data: snapshot }));
      ws.send(JSON.stringify({ type: "event", data: lap }));
      ws.send(JSON.stringify({ type: "event", data: crash }));
    },
    message() {},
  },
});
afterAll(() => server.stop(true));
const url = `http://127.0.0.1:${server.port}`;

test("a token is required", () => {
  expect(() => new GameSdk({ token: "" })).toThrow();
});

test("state() sends the token; a wrong one is a GameSdkError with the status", async () => {
  expect((await new GameSdk({ token: TOKEN, url }).state()).frame).toBe(7);
  const err = await new GameSdk({ token: "nope", url }).state().catch((e) => e);
  expect(err).toBeInstanceOf(GameSdkError);
  expect(err.status).toBe(401);
  expect(await new GameSdk({ token: "nope", url }).health()).toBe(true);
});

test("connect() accepts the current schema and refuses a different major", async () => {
  expect((await new GameSdk({ token: TOKEN, url }).connect()).schema).toBe(1);
  const err = await new GameSdk({ token: INCOMPATIBLE_TOKEN, url }).connect().catch((e) => e);
  expect(err).toBeInstanceOf(GameSdkError);
  expect(err.status).toBe(426);
});

test("stream() reports an incompatible WebSocket hello and closes", async () => {
  const stream = new GameSdk({ token: INCOMPATIBLE_TOKEN, url }).stream({ reconnect: false });
  const errors: GameSdkError[] = [];
  stream.onStatus((_connected, error) => error && errors.push(error));
  await new Promise((resolve) => setTimeout(resolve, 100));
  expect(errors[0]?.status).toBe(426);
  stream.close();
});

test("stream() routes frames, all events, and events of one type", async () => {
  const s = new GameSdk({ token: TOKEN, url }).stream({ rate: 500, events: true });
  const frames: Snapshot[] = [];
  const all: GameEvent[] = [];
  const laps: number[] = [];
  s.onState((f) => frames.push(f));
  s.onEvent((e) => all.push(e));
  s.onEvent("lap", (e) => laps.push(e.timeMs));
  await new Promise((r) => setTimeout(r, 300));
  s.close();
  expect(frames.map((f) => f.frame)).toEqual([7]);
  expect(all.map((e) => e.type)).toEqual(["lap", "crash"]);
  expect(laps).toEqual([95123]);
  expect(streamQuery).toContain("rate=60"); // clamped
  expect(streamQuery).toContain("topics=state,events");
});

test("formatLap", () => {
  expect(formatLap(95123)).toBe("1:35.123");
  expect(formatLap(5007)).toBe("0:05.007");
  expect(formatLap(null)).toBe("-:--.---");
});

test("act() sends the request, refusals are errors, waitForAction ends on done", async () => {
  const sdk = new GameSdk({ token: TOKEN, url });
  const sent = await sdk.refresh("tracks");
  expect(sent).toEqual({ id: "a1-x", action: "refresh", state: "sent" });
  const err = await sdk.act({ action: "reload_mods" }).catch((e) => e);
  expect(err.status).toBe(409);
  expect((await sdk.waitForAction("a1-x", 1000)).state).toBe("done");
});
