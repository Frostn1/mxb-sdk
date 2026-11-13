# Extensions

`mxb-extensions` and `@mxb/extensions` contain the portable extension package contract. The kit
exports manifest and permission types, `validateManifest`, `pack`, `keygen`, and `verify`.

```ts
import { keygen, pack, validateManifest, verify } from "@mxb/extensions";

const checked = validateManifest(JSON.parse(source));
keygen("author.pem");
pack("my-extension", { key: "author.pem" });
verify("yourname.my-extension-1.0.0.mxbext");
```

The CLI provides the same operations:

```bash
mxb ext validate extension.json --json
mxb ext keygen author.pem --json
mxb ext pack my-extension --key author.pem --json
mxb ext verify yourname.my-extension-1.0.0.mxbext --json
```

## Enforced package rules

Packages may contain only JSON, HTML, JavaScript, CSS, PNG, JPEG, WebP, GIF, SVG, WOFF, WOFF2,
TTF, OTF, text, and Markdown files. A compressed package is limited to 25 MiB, each unpacked
file to 10 MiB, the unpacked total to 60 MiB, and the package to 2,000 files.

Manifests may request only these permissions:

- `mods.read` — read mod information.
- `servers.read` — read server information.
- `friends.presence` — read other players' presence; it cannot be combined with `network`.
- `game.state` — read the current game state.
- `game.read` — read game data and streams.
- `presets.read` — read presets.
- `coach.read` — read coaching data.
- `studio.launch` — launch the studio.
- `network` — fetch from up to ten HTTPS origins listed by the manifest.
- `openExternal` — open an external address.
- `mcp.tools` — use contributed tool results.

The `.mxbext` signature covers every accepted file path and SHA-256 digest, so any changed byte
fails verification. A package is `verified` when its Ed25519 key is in the host's trusted
publisher list, `signed` when the signature is valid but the key is not trusted, and `unsigned`
when it has no signature. Unsigned packages require developer mode. The signing-key fingerprint
is pinned at installation: updates must keep that key, except that a host-verified publisher key
may replace it; developer mode also permits the explicit unsigned transitions implemented by the
installer. IDs with host-reserved prefixes require a verified publisher signature.
