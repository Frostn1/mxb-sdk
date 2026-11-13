import { afterAll, expect, test } from "bun:test";
import { McpClient } from "../src/index";

let requestCount = 0;
const server = Bun.serve({
  port: 0,
  async fetch(request) {
    requestCount++;
    if (request.headers.get("authorization") !== "Bearer test-token") return new Response("no", { status: 401 });
    const message = await request.json() as { id: number; method: string; params: Record<string, unknown> };
    let result: unknown;
    if (message.method === "initialize") result = { protocolVersion: "2025-03-26", capabilities: { tools: {} }, serverInfo: { name: "MXB App", version: "1.2.0" } };
    else if (message.method === "tools/list") result = { tools: [{ name: "get_game_state", inputSchema: { type: "object" } }, { name: "change_game" }] };
    else result = { content: [{ type: "text", text: JSON.stringify({ method: message.method, ...message.params }) }] };
    return Response.json({ jsonrpc: "2.0", id: message.id, result });
  },
});
afterAll(() => server.stop(true));

test("initializes, lists tools, and calls one", async () => {
  const client = new McpClient({ token: "test-token", url: `http://127.0.0.1:${server.port}` });
  expect((await client.initialize()).serverInfo.name).toBe("MXB App");
  expect((await client.listTools())[0]?.name).toBe("get_game_state");
  const result = await client.callTool("get_game_state", { detail: true });
  expect(result.content[0]?.text).toContain("get_game_state");
});

test("filters tools and refuses a write tool without another HTTP request", async () => {
  const client = new McpClient({ token: "test-token", url: `http://127.0.0.1:${server.port}` });
  expect((await client.listTools()).map((tool) => tool.name)).toEqual(["get_game_state"]);
  const requestsBefore = requestCount;
  const error = await client.callTool("change_game").catch((caught) => caught);
  expect(error.code).toBe(-32601);
  expect(error.message).toBe("tool not available: change_game (the SDK is read-only)");
  expect(requestCount).toBe(requestsBefore);
});

async function bridge(lines: string[]): Promise<string> {
  const proc = Bun.spawn(["node", new URL("../bin/mxb-mcp.mjs", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"), "--url", `http://127.0.0.1:${server.port}`, "--token", "test-token"], {
    stdin: "pipe",
    stdout: "pipe",
  });
  proc.stdin.write(lines.join("\n") + "\n");
  proc.stdin.end();
  const out = await new Response(proc.stdout).text();
  await proc.exited;
  return out;
}

test("the stdio bridge refuses write tools and batches without an HTTP request", async () => {
  const before = requestCount;
  const out = await bridge([
    JSON.stringify({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "change_game" } }),
    JSON.stringify([{ jsonrpc: "2.0", id: 2, method: "tools/call", params: { name: "change_game" } }]),
  ]);
  expect(out).toContain("-32601");
  expect(out).toContain("-32600");
  expect(requestCount).toBe(before);
});

test("the stdio bridge filters tools/list to read-only tools", async () => {
  const out = await bridge([JSON.stringify({ jsonrpc: "2.0", id: 3, method: "tools/list" })]);
  expect(out).toContain("get_game_state");
  expect(out).not.toContain("change_game");
});
