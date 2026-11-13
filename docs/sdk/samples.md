# Samples

| Sample | Language | What it shows |
|---|---|---|
| [Overlay](../../packages/mxb-game/samples/overlay/index.html) | HTML/JavaScript | OBS overlay over the WebSocket stream |
| [Lap timer extension](../../extensions/samples/lap-timer/extension.json) | HTML/JavaScript | Sandboxed extension using live game data |
| [Node laps](../../packages/mxb-game/samples/node/laps.mjs) | JavaScript | Laps, passes, crashes, and a summary |
| [Node session logger](../../packages/mxb-game/samples/node/session-logger.mjs) | JavaScript | `node packages/mxb-game/samples/node/session-logger.mjs` writes laps, results, and optional telemetry |
| [Node Discord laps](../../packages/mxb-game/samples/node/discord-laps.mjs) | JavaScript | `node packages/mxb-game/samples/node/discord-laps.mjs` posts personal bests and final timing |
| `crates/mxb-game/examples/lap_logger.rs` | Rust | Typed live events and lap formatting |
| `crates/mxb-game/examples/session_logger.rs` | Rust | `cargo run -p mxb-game --example session_logger` writes lap CSV files |
| `crates/mxb-game/examples/discord_laps.rs` | Rust | `cargo run -p mxb-game --example discord_laps` posts personal bests and final timing |
| `crates/mxb-core/examples/list_and_install.rs` | Rust | Find an install, inspect a plan, and apply it |

The browser and Node samples need the local API token.
