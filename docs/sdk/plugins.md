# Plugins

`mxb-plugins` and `@mxb/plugins` describe optional metadata beside an MX Bikes `.dlo` plugin and
list active or disabled plugins. Put `<plugin-name>.mxbplugin.json` beside the `.dlo`; every
manifest field is optional and unknown fields are ignored.

| Field | Type and limits |
|---|---|
| `name` | Display name, at most 60 characters |
| `version` | Version text, at most 32 characters |
| `author` | Author name, at most 60 characters |
| `description` | Plain-text summary, at most 400 characters |
| `category` | `hud`, `graphics`, `physics`, `fixes`, `timing`, or `other` |
| `homepage` | An `https://` URL, at most 300 characters |
| `gameVersion` | Compatible game version, at most 16 characters |
| `files` | Up to 64 safe relative files or folders that belong to the plugin |
| `settings` | Up to 16 settings files; each has `file`, optional `label` (40 characters), and optional `where` (`plugins` or `documents`) |
| `keys` | Up to 16 shortcuts; each has `key` (24 characters) and `label` (80 characters) |
| `racing` | `off`, `stay`, or `always-on` |
| `conflicts` | Up to 16 entries with `with` (80 characters) and `reason` (200 characters) |
| `updates` | `{ "github": "owner/repo" }` release source |

The manifest must be no larger than 64 KB. Relative paths cannot be absolute, contain `.` or
`..` components, or name protected game files.

```json
{
  "name": "Example HUD",
  "version": "1.2.0",
  "author": "Example Author",
  "description": "A compact race HUD.",
  "category": "hud",
  "homepage": "https://example.com/plugin",
  "gameVersion": "beta21e",
  "files": ["example_data/"],
  "settings": [{ "file": "example.ini", "label": "HUD settings", "where": "plugins" }],
  "keys": [{ "key": "Ctrl+F7", "label": "Toggle HUD" }],
  "racing": "stay",
  "conflicts": [{ "with": "other-hud", "reason": "Both draw in the same area." }],
  "updates": { "github": "owner/example-hud" }
}
```

```ts
import { listPlugins } from "@mxb/plugins";

for (const plugin of await listPlugins("C:/Games/MX Bikes/plugins")) {
  console.log(plugin.id, plugin.enabled, plugin.manifest?.version);
}
```

Disabled plugins are stored under the SDK's disabled-plugin folder and retain the same manifest
and owned-file metadata.
