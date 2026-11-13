# mxb-sdk

`mxb-sdk` is the umbrella for the public MX Bikes developer packages. Use the umbrella when one
project spans several areas. Use a smaller crate or npm package when it does one job.

| Package | Rust crate | npm | What it is for | Status |
|---|---|---|---|---|
| Core | `mxb-core` | `@mxb/core` | Folders, mod inventory, identify, install, uninstall, verify, paints, presets, and plugins | 0.1 |
| Game | `mxb-game` | `@mxb/game` | Read-only live state, events, laps, and schema types | 1.2 |
| Extensions | `mxb-extensions` | `@mxb/extensions` | Manifest types, validator, packer, key generation, and verification | 0.1 |
| Plugins | `mxb-plugins` | `@mxb/plugins` | Game plugin manifest types and folder reader | 0.1 |
| MCP | `mxb-mcp` | `@mxb/mcp` | Stdio bridge and typed HTTP client | 0.1 |
| Umbrella | `mxb-sdk` | `@mxb/sdk` | One dependency and grouped exports | 0.1 |

## Install from this checkout

The packages are private while their APIs settle. Rust projects can use a path dependency:

```toml
[dependencies]
mxb-sdk = { path = "../mxb-app/crates/mxb-sdk", features = ["core", "game"] }
```

TypeScript projects in this monorepo use the workspace package:

```json
{ "dependencies": { "@mxb/sdk": "workspace:*" } }
```

## Pick your package

- Managing files and installed content: [Core](core.md).
- Reading a running session or building an overlay: [Game](game.md).
- Building and signing an MXB App extension: [Extensions](extensions.md).
- Inspecting game plugin files: [Plugins](plugins.md).
- Connecting an AI client: [MCP](mcp.md).
- Using more than one area: [Rust umbrella](rust.md) or [TypeScript umbrella](typescript.md).

## Safety rules

- Live game data is read-only and filtered to stay fair online. Nothing in the clients sends
  input, commands, or settings to the game.
- The SDK never deletes user files. Uninstall operations move them to the operating system trash.
- The SDK does not decode protected or private file formats.

See [versioning and compatibility](versions.md), the [CLI](cli.md), and the [samples](samples.md).
