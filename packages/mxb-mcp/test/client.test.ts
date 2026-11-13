import { afterAll, expect, test } from "bun:test";
import { McpClient } from "../src/index";

const server = Bun.serve({
  port: 0,
  async fetch(request) {
    if (request.headers.get("authorization") !== "Bearer test-token") return new Response("no", { status: 401 });
    const message = await request.json() as { id: number; method: string; params: Record<string, unknown> };
    let result: unknown;
    if (message.method === "initialize") result = { protocolVersion: "2025-03-26", capabilities: { tools: {} }, serverInfo: { name: "MXB App", version: "1.1.0" } };
    else if (message.method === "tools/list") result = { tools: [{ name: "get_game_state", inputSchema: { type: "object" } }] };
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
