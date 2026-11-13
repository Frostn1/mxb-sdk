# @mxb/game

Live MX Bikes data through MXB App's Game SDK: typed schemas, a client for browsers and Node 22+,
and a compatibility command for the MCP bridge. Read-only, and fair online.

Full reference: [docs/sdk/game.md](../../docs/sdk/game.md). Versioning, compatibility, and the
handshake are documented in [docs/sdk/versions.md](../../docs/sdk/versions.md).

## Use it

Turn on **MXB App → Settings → Game SDK → Local API** and copy the token.

```ts
import { GameSdk, formatLap } from "@mxb/game";

const mxb = new GameSdk({ token: "YOUR_TOKEN" });

const now = await mxb.state();
console.log(now.session?.trackName, now.local?.speedKmh);

const stream = mxb.stream({ rate: 30 });
stream.onState((s) => console.log(s.local?.gear, s.local?.rpm));
stream.onEvent("lap", (e) => e.local && console.log(`Lap ${e.lap}: ${formatLap(e.timeMs)}`));
stream.onEvent("overtake", (e) => e.local && console.log(`Up to P${e.place}`));
```

| | |
|---|---|
| `state()` | the current snapshot |
| `hello()`, `connect()` | server versions and capabilities; `connect()` rejects an incompatible schema |
| `events({ since })` | recent events after a `seq` |
| `laps()` | your laps this session |
| `stream({ rate, state, events })` | live frames and events over a WebSocket, reconnecting |
| `health()` | whether the app's endpoint is up |

## Develop without the game

```bash
bun run build
node bin/mxb-game-mock.mjs --online --speed 10
```

The mock prints its token and replays a synthetic three-lap race. It also accepts
`--fixture session.jsonl`; `recordFixture(sdk, file, { seconds })` records your own stream.

## Build

Not published to npm yet.

```bash
bun run build      # dist/
bun test
```

## Samples

- `samples/overlay/index.html`: a stream overlay for OBS (`?token=…`)
- `samples/node/laps.mjs`: laps, passes and crashes in a terminal
- `samples/node/session-logger.mjs`: lap CSV, final results, and optional local telemetry
- `samples/node/discord-laps.mjs`: personal bests and final timing through a Discord webhook
- `../../extensions/samples/lap-timer`: an MXB App extension tab

## MCP

The compatibility `mxb-game-mcp` command starts the bridge from `@mxb/mcp`:

```bash
MXB_GAME_TOKEN=YOUR_TOKEN mxb-game-mcp
```

Clients that speak HTTP can use `http://127.0.0.1:47820/mcp` with `Authorization: Bearer YOUR_TOKEN`
directly.
