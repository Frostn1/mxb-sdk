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

createInterface({ input: process.stdin }).on("line", async (line) => {
  if (!line.trim()) return;
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return write({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "parse error" } });
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
    if (body) process.stdout.write(body.trim() + "\n");
  } catch (error) {
    if (message.id !== undefined) {
      const detail = error instanceof Error ? error.message : String(error);
      write({ jsonrpc: "2.0", id: message.id, error: { code: -32000, message: `MXB App is not reachable at ${url}: is the Game SDK local API on? (${detail})` } });
    }
  }
});
