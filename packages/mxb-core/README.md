# @mxb/core

Typed Node wrappers for the public `mxb mods` commands.

```ts
import { findInstalls, listMods } from "@mxb/core";

const installs = await findInstalls();
const bikes = await listMods("bikes");
```

Set `MXB_BIN` to an explicit CLI path. Otherwise the wrapper runs `mxb` from `PATH`. Every call
uses `mxb mods <command> --json`.
