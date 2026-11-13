# @mxb/sdk

One import for the MX Bikes developer packages.

| Package | Import | Purpose |
|---|---|---|
| Core | `core` | Folders and mod management through `mxb mods` |
| Game | `game` | Live game state, events, actions, and types |
| Extensions | `extensions` | Manifest validation, packing, keys, and verification |
| Plugins | `plugins` | Game plugin manifests and folder reading |
| MCP | `mcp` | HTTP client and stdio bridge |

```ts
import { GameSdk, findInstalls, formatLap } from "@mxb/sdk";
```

The package also exports `core`, `game`, `extensions`, `plugins`, and `mcp` namespaces.
