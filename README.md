# mxb-sdk

Free, open tools for building on MX Bikes: find the game and its mods, install and check mods,
read the live game through MXB App, build MXB App extensions, describe game plugins, and give
AI assistants the game's data.

| Package | Rust crate | npm | For |
|---|---|---|---|
| core | `mxb-core` | `@mxb/core` | Find installs, list, identify, install, uninstall and verify mods; paints and presets |
| game | `mxb-game` | `@mxb/game` | Live game data and events from MXB App; reload mods in the running game |
| extensions | `mxb-extensions` | `@mxb/extensions` | Build, check, pack and sign MXB App extensions |
| plugins | `mxb-plugins` | `@mxb/plugins` | MX Bikes game plugins and the `.mxbplugin.json` manifest |
| mcp | `mxb-mcp` | `@mxb/mcp` | The MCP bridge for AI assistants |
| everything | `mxb-sdk` | `@mxb/sdk` | One dependency with features, and the `mxb` command line |

```toml
[dependencies]
mxb-sdk = { version = "0.1", features = ["core", "game"] }
```

```bash
npm install @mxb/sdk
```

Start with the docs: [docs/sdk/README.md](docs/sdk/README.md).

## Promises

- The game client is read-only, and fair online: other riders are what the game's timing screen
  shows, nothing more.
- Installing never deletes your files; removing a mod sends it to the Recycle Bin.
- No private formats, keys or network services. Everything here works offline except the live
  game client, which talks only to MXB App on your own PC (127.0.0.1).

## This repository

It is published automatically from the private monorepo MXB App is built in, so a change here
arrives as a pull request upstream rather than being merged directly. See
[CONTRIBUTING.md](CONTRIBUTING.md).

MIT licensed. Not affiliated with PiBoSo; MX Bikes is PiBoSo's.
