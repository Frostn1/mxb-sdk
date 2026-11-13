# TypeScript packages

All npm packages are ESM and private while the APIs settle.

| npm package | Main exports |
|---|---|
| `@mxb/core` | `findInstalls`, `listMods`, `identify`, `plan`, `install`, `uninstall`, `verify` |
| `@mxb/game` | `GameSdk`, schema types, `formatLap` |
| `@mxb/extensions` | manifest types, `validateManifest`, `pack`, `keygen`, `verify` |
| `@mxb/plugins` | manifest types, `listPlugins`, `readPluginManifest` |
| `@mxb/mcp` | `McpClient`, `mxb-mcp` |
| `@mxb/sdk` | all five namespaces and common top-level exports |

```ts
import { GameSdk, findInstalls, formatLap } from "@mxb/sdk";

const installs = await findInstalls();
const game = new GameSdk({ token: process.env.MXB_GAME_TOKEN! });
game.stream().onEvent("lap", (lap) => console.log(formatLap(lap.timeMs)));
```

Use direct imports when a project needs one area:

```ts
import { validateManifest } from "@mxb/extensions";
import type { Snapshot } from "@mxb/game";
```
