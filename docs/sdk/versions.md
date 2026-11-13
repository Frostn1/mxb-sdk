# SDK versions

The developer packages use semantic versions independently. The game protocol is `1.2.0`; the
new core, extensions, plugins, MCP, and umbrella packages begin at `0.1.0`. During `0.x`, a breaking
change increments the minor version. Package compatibility is recorded in the shared
[changelog](CHANGELOG.md).

Removed: game actions. The packages were never published, so they are removed before the first
release instead of deprecated.

## Game protocol

The Game SDK uses semantic versions. A patch release fixes an implementation or documentation
error without changing the public schema. A minor release adds backward-compatible fields,
events, capabilities, or client methods. A major release may remove or change public shapes or
behaviour.

## Schema and handshake

The integer schema version is the compatibility boundary for serialized data. Its major is kept
in step with the SDK major: SDK 1.x uses schema 1. New optional fields can appear within one schema
major, so consumers should ignore fields they do not know.

Authenticated clients can call `GET /v1/hello` before using the API. It returns the schema and SDK
versions, the MXB App version, enabled capabilities, game integration state, and current deprecations. The
first WebSocket message carries the same fields plus the requested rate. The TypeScript client's
`connect()` checks the HTTP handshake, and `stream()` checks the WebSocket handshake. Both report a
426 error when the schema major is incompatible.

## Deprecation policy

Before a public field, event, or operation is removed, its path is added to the schema's
`x-deprecated` array and documented for at least one minor release and two months. Removal happens
only in the next major release. A removed operation answers with a clear error instead of silently
doing something else.

## Regenerating types

Edit `schema/game-sdk.schema.json`, then run:

```bash
node scripts/gamesdk/codegen.mjs
```

This writes the TypeScript and Rust public types. CI runs
`node scripts/gamesdk/codegen.mjs --check` to catch stale generated files.

## Game compatibility

| MX Bikes build | plugin API data version | game integration | feed (read) | notes |
|---|---:|---|---|---|
| beta21e | 8 | 0.53+ | yes | Supported through the published plugin API. |
| Other builds | 8 | 0.53+ | yes | The feed uses only the published plugin API. |
| GP Bikes and Kart Racing Pro | other game layouts | carries the data | not decoded | The feed carries their data, but the app does not decode it yet. |
