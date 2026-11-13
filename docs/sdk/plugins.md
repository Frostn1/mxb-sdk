# Plugins

`mxb-plugins` and `@mxb/plugins` describe plugin manifests and read an MX Bikes `plugins` folder.
The reader recognizes enabled `.dlo` and `.dll` files. A `.dlo.bak` or `.dll.bak` file is the same
plugin in its disabled state. Names exclude both suffixes.

```ts
import { listPlugins } from "@mxb/plugins";

for (const plugin of await listPlugins("C:/Games/MX Bikes/plugins")) {
  console.log(plugin.name, plugin.kind, plugin.enabled, plugin.manifest?.version);
}
```

Public manifest metadata is `id`, `name`, `version`, `entry`, optional `minAppVersion`, `host`, and
`panels`. The SDK does not expose entitlement or licence state.

```bash
mxb plugins list "C:/Games/MX Bikes/plugins" --json
mxb plugins read "C:/Games/MX Bikes/plugins/alpha.json" --json
```
