#!/usr/bin/env node
// Bridge MXB App's HTTP MCP endpoint to JSON-RPC lines on stdio.
//
//   MXB_GAME_TOKEN=<token> mxb-mcp
//   options: --url http://127.0.0.1:47820 --token <token>
import { createInterface } from "node:readline";

const arg = (name) => {
  const index = process.argv.indexOf(`--${name}`);
  return index > 0 ? process.argv[index + 1] : undefined;
};
const url = (arg("url") ?? process.env.MXB_GAME_URL ?? "http://127.0.0.1:47820").replace(/\/+$/, "") + "/mcp";
const token = arg("token") ?? process.env.MXB_GAME_TOKEN;
if (!token) {
  process.stderr.write("mxb-mcp: set MXB_GAME_TOKEN (MXB App -> Settings -> Game SDK)\n");
  process.exit(2);
}

const write = (message) => process.stdout.write(JSON.stringify(message) + "\n");
const READ_ONLY_TOOLS = new Set([
  "get_game_state",
  "get_last_lap",
  "get_session_laps",
  "get_standings",
  "get_recent_events",
]);

createInterface({ input: process.stdin }).on("line", async (line) => {
  if (!line.trim()) return;
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return write({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "parse error" } });
  }
  // A batch could carry a tools/call past the check below, so batches are refused.
  if (Array.isArray(message) || message === null || typeof message !== "object") {
    return write({ jsonrpc: "2.0", id: null, error: { code: -32600, message: "batches are not supported by the read-only bridge" } });
  }
  if (message.method === "tools/call" && !READ_ONLY_TOOLS.has(message.params?.name)) {
    return write({ jsonrpc: "2.0", id: message.id ?? null, error: { code: -32601, message: `tool not available: ${message.params?.name ?? ""} (the SDK is read-only)` } });
  }
  try {
    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json", Accept: "application/json", Authorization: `Bearer ${token}` },
      body: JSON.stringify(message),
    });
    if (response.status === 202) return;
    const body = await response.text();
    if (!response.ok && message.id !== undefined) {
      return write({ jsonrpc: "2.0", id: message.id, error: { code: -32000, message: `MXB App answered ${response.status}: ${body.slice(0, 200)}` } });
    }
    if (body) {
      if (message.method === "tools/list") {
        const result = JSON.parse(body);
        if (Array.isArray(result.result?.tools)) result.result.tools = result.result.tools.filter((tool) => READ_ONLY_TOOLS.has(tool.name));
        return write(result);
      }
      process.stdout.write(body.trim() + "\n");
    }
  } catch (error) {
    if (message.id !== undefined) {
      const detail = error instanceof Error ? error.message : String(error);
      write({ jsonrpc: "2.0", id: message.id, error: { code: -32000, message: `MXB App is not reachable at ${url}: is the Game SDK local API on? (${detail})` } });
    }
  }
});
