export interface McpClientOptions {
  token: string;
  /** App base URL or the full `/mcp` URL. */
  url?: string;
  clientName?: string;
  clientVersion?: string;
}

export interface McpTool {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
}

export interface McpCallResult {
  content: Array<{ type: string; text?: string; [key: string]: unknown }>;
  isError?: boolean;
  structuredContent?: unknown;
}

/** Tool names the read-only client permits. */
export const READ_ONLY_TOOLS = [
  "get_game_state",
  "get_last_lap",
  "get_session_laps",
  "get_standings",
  "get_recent_events",
] as const;

const readOnlyTools = new Set<string>(READ_ONLY_TOOLS);

interface JsonRpcResponse<T> {
  jsonrpc: "2.0";
  id: number;
  result?: T;
  error?: { code: number; message: string; data?: unknown };
}

export class McpError extends Error {
  constructor(message: string, readonly code: number, readonly data?: unknown) { super(message); }
}

/** Typed Streamable HTTP client for MXB App's local MCP endpoint. */
export class McpClient {
  readonly url: string;
  private readonly token: string;
  private readonly clientName: string;
  private readonly clientVersion: string;
  private nextId = 1;
  private initialized = false;

  constructor(options: McpClientOptions) {
    if (!options?.token) throw new Error("a token is required");
    const base = (options.url ?? "http://127.0.0.1:47820").replace(/\/+$/, "");
    this.url = base.endsWith("/mcp") ? base : `${base}/mcp`;
    this.token = options.token;
    this.clientName = options.clientName ?? "@mxb/mcp";
    this.clientVersion = options.clientVersion ?? "0.1.0";
  }

  private async request<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    const id = this.nextId++;
    const response = await fetch(this.url, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${this.token}`,
        Accept: "application/json, text/event-stream",
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ jsonrpc: "2.0", id, method, params }),
    });
    const body = await response.text();
    if (!response.ok) throw new McpError(`MCP HTTP ${response.status}: ${body.slice(0, 200)}`, response.status);
    const message = JSON.parse(body) as JsonRpcResponse<T>;
    if (message.error) throw new McpError(message.error.message, message.error.code, message.error.data);
    if (message.result === undefined) throw new McpError("MCP response has no result", -32603);
    return message.result;
  }

  async initialize(): Promise<{ protocolVersion: string; capabilities: Record<string, unknown>; serverInfo: { name: string; version: string } }> {
    const result = await this.request<{
      protocolVersion: string;
      capabilities: Record<string, unknown>;
      serverInfo: { name: string; version: string };
    }>("initialize", {
      protocolVersion: "2025-03-26",
      capabilities: {},
      clientInfo: { name: this.clientName, version: this.clientVersion },
    });
    this.initialized = true;
    return result;
  }

  async listTools(): Promise<McpTool[]> {
    if (!this.initialized) await this.initialize();
    return (await this.request<{ tools: McpTool[] }>("tools/list")).tools.filter((tool) => readOnlyTools.has(tool.name));
  }

  async callTool(name: string, args: Record<string, unknown> = {}): Promise<McpCallResult> {
    if (!readOnlyTools.has(name)) throw new McpError(`tool not available: ${name} (the SDK is read-only)`, -32601);
    if (!this.initialized) await this.initialize();
    return this.request<McpCallResult>("tools/call", { name, arguments: args });
  }
}
