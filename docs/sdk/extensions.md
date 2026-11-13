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

The `.mxbext` signature covers every accepted file path and SHA-256 digest. A changed byte fails
verification. The full sandbox, contribution, permission, and publishing rules remain in the
[extension author guide](../extensions.md). See also [building with AI agents](../extensions-ai-agents.md).
