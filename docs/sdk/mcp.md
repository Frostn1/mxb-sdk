# MCP

MXB App serves MCP over Streamable HTTP at `http://127.0.0.1:47820/mcp`. It needs the local API
token. `mxb-mcp` is the stdio bridge for clients that launch a process. `McpClient` is the typed
HTTP client for Node and Bun.

The game server and stdio bridge expose only `get_game_state`, `get_last_lap`, `get_session_laps`,
`get_standings`, and `get_recent_events`.

Claude Code can connect directly:

```bash
claude mcp add --transport http mxb-game http://127.0.0.1:47820/mcp --header "Authorization: Bearer YOUR_TOKEN"
```

Claude Desktop and Cursor can launch the bridge:

```json
{
  "mcpServers": {
    "mxb-game": {
      "command": "mxb-mcp",
      "env": { "MXB_GAME_TOKEN": "YOUR_TOKEN" }
    }
  }
}
```

```ts
import { McpClient } from "@mxb/mcp";

const client = new McpClient({ token: process.env.MXB_GAME_TOKEN! });
await client.initialize();
console.log(await client.listTools());
```

See [Game SDK MCP tools](game.md#ai-assistants-mcp) for response details.
