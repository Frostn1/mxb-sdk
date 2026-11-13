# @mxb/plugins

Types for plugin manifests and a Node reader for the game's `plugins` folder.

```ts
import { listPlugins } from "@mxb/plugins";

const plugins = await listPlugins("C:/Games/MX Bikes/plugins");
```

The reader recognizes active `.dlo` files in `plugins` and disabled files in
`mxbapp_disabled/plugins`. A matching `<name>.mxbplugin.json` is read when present; stock game
shims are never listed.
