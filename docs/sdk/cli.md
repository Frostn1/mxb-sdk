# CLI

The `mxb` binary groups commands by package. Every command accepts `--json`, including commands
whose normal output is already short.

## Mods (`core` feature)

```bash
mxb mods folders --json
mxb mods list [--kind bikes|tracks|rider|paints|tyres|sounds|plugins|misc] --json
mxb mods identify <path> --json
mxb mods plan <source> --json
mxb mods install <source> --json
mxb mods uninstall <mod-path> --json
mxb mods verify [<mod-path>] --json
mxb mods duplicates --json
mxb mods paints <item> --json
```

## Game (`game` feature)

The token is in MXB App -> Settings -> Game SDK. Set `MXB_GAME_TOKEN` (and `MXB_GAME_URL` if the
port is not 47820).

```bash
mxb game hello --json
mxb game state --json
mxb game watch            # laps, passes, crashes and action results as they happen
mxb game laps --json
mxb game reload --json    # needs "Allow actions"
mxb game refresh tracks|bikes|gear|paints|all --json
```

## Extensions (`extensions` feature)

```bash
mxb ext validate <folder> --json
mxb ext pack <folder> [--key author.pem] --json
mxb ext keygen <author.pem> --json
```

## Plugins (`plugins` feature)

```bash
mxb plugins list [<plugins folder>] --json
mxb plugins inspect <name.mxbplugin.json> --json
```

## MCP (`mcp` feature)

```bash
mxb mcp        # stdio MCP bridge to MXB App; reads MXB_GAME_TOKEN and MXB_GAME_URL
```
