import { afterAll, beforeAll, expect, test } from "bun:test";
import { mkdir, readFile, readdir, rm } from "node:fs/promises";
import { request } from "node:http";
import { join, resolve } from "node:path";
import { GameSdk, startMockServer, type GameEvent, type Snapshot } from "../src/index";

let mock: Awaited<ReturnType<typeof startMockServer>>;
beforeAll(async () => { mock = await startMockServer({ port: 0, speed: 100 }); });
afterAll(async () => { await mock.close(); });

test("REST API works with the real client", async () => {
  const sdk = new GameSdk({ token: mock.token, url: mock.url });
  expect((await sdk.hello()).app).toBe("mock");
  expect((await sdk.state()).session?.trackId).toBe("example");
  await Bun.sleep(1100);
  expect((await sdk.events()).events.length).toBeGreaterThan(0);
  expect((await sdk.laps()).length).toBeGreaterThan(0);
  expect((await fetch(`${mock.url}/v1/state`)).status).toBe(401);
  expect((await fetch(`${mock.url}/v1/state`, { method: "POST", headers: { authorization: `Bearer ${mock.token}` } })).status).toBe(405);
});

test("bad Host is forbidden", async () => {
  const status = await new Promise<number>((done, reject) => {
    const req = request(mock.url + "/v1/health", { headers: { host: "bad.example" } }, (res) => { res.resume(); done(res.statusCode ?? 0); });
    req.on("error", reject); req.end();
  });
  expect(status).toBe(403);
});

test("stream starts with hello and delivers state and lap", async () => {
  const sdk = new GameSdk({ token: mock.token, url: mock.url });
  const stream = sdk.stream({ rate: 60, reconnect: false });
  let connected = false; const states: Snapshot[] = []; const events: GameEvent[] = [];
  stream.onStatus((up) => { connected ||= up; }); stream.onState((s) => states.push(s)); stream.onEvent((e) => events.push(e));
  await Bun.sleep(1400); stream.close();
  expect(connected).toBe(true); expect(states.length).toBeGreaterThan(0); expect(events.some((e) => e.type === "lap")).toBe(true);
});

test("online fairness hides remote track and remote crash events", async () => {
  const online = await startMockServer({ port: 0, online: true, speed: 100 });
  try {
    const sdk = new GameSdk({ token: online.token, url: online.url }); await Bun.sleep(1500);
    const state = await sdk.state(); expect(state.riders.filter((r) => !r.local).every((r) => r.track === null)).toBe(true);
    const events = (await sdk.events()).events; expect(events.some((e) => (e.type === "crash" || e.type === "recover") && !e.local)).toBe(false);
  } finally { await online.close(); }
});

test("MCP exposes only five read tools and standings", async () => {
  const call = async (method: string, params?: unknown) => (await fetch(mock.url + "/mcp", {
    method: "POST", headers: { authorization: `Bearer ${mock.token}`, "content-type": "application/json" }, body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  })).json();
  const list = await call("tools/list");
  expect(list.result.tools.map((tool: { name: string }) => tool.name)).toEqual(["get_game_state", "get_last_lap", "get_session_laps", "get_standings", "get_recent_events"]);
  const standings = await call("tools/call", { name: "get_standings", arguments: {} });
  expect(JSON.parse(standings.result.content[0].text).riders.length).toBe(8);
});

async function lapRows(folder: string): Promise<number> {
  let rows = 0;
  for (const f of (await readdir(folder)).filter((name) => name.endsWith(".csv")))
    rows += (await readFile(join(folder, f), "utf8")).trim().split("\n").length - 1;
  return rows;
}

test("Node logger and Discord samples run against the mock", async () => {
  // The samples run under Node, so the mock runs under Node too (a separate process), the way a user runs them.
  const cwd = resolve(import.meta.dir, "..");
  const folder = join(cwd, ".test-tmp", `${process.pid}-${Date.now()}`); await mkdir(folder, { recursive: true }); const posts: unknown[] = [];
  const webhook = Bun.serve({ port: 0, async fetch(req) { posts.push(await req.json()); return new Response("ok"); } });
  const server = Bun.spawn(["node", "bin/mxb-game-mock.mjs", "--port", "0", "--token", "sample-test", "--speed", "50"], { cwd, stdout: "pipe", stderr: "pipe" });
  const children = [server];
  try {
    const reader = (server.stdout as ReadableStream<Uint8Array>).getReader();
    let banner = "";
    while (!/http:\/\/127\.0\.0\.1:\d+/.test(banner)) {
      const { value, done } = await reader.read();
      if (done) throw new Error("mock exited: " + (await new Response(server.stderr as ReadableStream).text()));
      banner += new TextDecoder().decode(value);
    }
    reader.releaseLock();
    const url = banner.match(/http:\/\/127\.0\.0\.1:\d+/)![0];
    const env = { ...process.env, MXB_GAME_TOKEN: "sample-test", MXB_GAME_URL: url,
      DISCORD_WEBHOOK_URL: `http://127.0.0.1:${webhook.port}/hook`, MXB_TEST_WEBHOOK_ALLOW_LOCAL: "1" };
    children.push(Bun.spawn(["node", "samples/node/session-logger.mjs", folder], { cwd, env, stdout: "ignore", stderr: "inherit" }));
    children.push(Bun.spawn(["node", "samples/node/discord-laps.mjs"], { cwd, env, stdout: "ignore", stderr: "inherit" }));
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline) {
      if ((await lapRows(folder)) >= 1 && posts.length) break;
      await Bun.sleep(100);
    }
    // The fixture loops, so the logger may have started a new file; count rows across all of them.
    expect((await readdir(folder)).some((f) => f.endsWith(".csv"))).toBe(true);
    expect(await lapRows(folder)).toBeGreaterThanOrEqual(1);
    expect(posts.some((body) => typeof body === "object" && body !== null && "content" in body)).toBe(true);
  } finally {
    for (const child of children) child.kill();
    webhook.stop(true); await rm(folder, { recursive: true, force: true });
  }
}, 25_000);
