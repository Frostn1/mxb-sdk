# @mxb/mcp

The `mxb-mcp` command bridges stdio MCP clients to MXB App's local HTTP endpoint. The package also
exports a typed HTTP client.

```ts
import { McpClient } from "@mxb/mcp";

const client = new McpClient({ token: process.env.MXB_GAME_TOKEN! });
console.log(await client.listTools());
```

```bash
MXB_GAME_TOKEN=YOUR_TOKEN mxb-mcp
```
