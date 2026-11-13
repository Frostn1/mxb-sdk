# @mxb/extensions

Types and tools for MXB App extension packages.

```ts
import { validateManifest, keygen, pack, verify } from "@mxb/extensions";

const result = validateManifest(JSON.parse(source));
await keygen("author.pem");
await pack("my-extension", { key: "author.pem" });
await verify("yourname.my-extension-1.0.0.mxbext");
```

The packer uses the same ZIP layout and Ed25519 digest as MXB App. It has no runtime dependencies.
