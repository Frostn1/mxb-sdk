# mxb-core

`mxb-core` is MXB App's public, headless library for the files and folders around MX
Bikes. It can be embedded without Tauri and the app uses the same install detection and
routing logic.

## Install

The crate is a path dependency while its API settles:

```toml
[dependencies]
mxb-core = { path = "../../crates/mxb-core" }
```

## Rust API

- `folders::find_installs()` — find Steam, manual, and Proton installs: `let games = folders::find_installs();`.
- `mods::list()` — inventory installed content: `let bikes = mods::list(root, Some(Kind::Bikes));`.
- `identify::identify()` — identify a folder or ordinary archive: `let found = identify::identify(path)?;`.
- `install::plan()` — produce every destination before writing: `let plan = install::plan(src, mods, None)?;`.
- `install::apply()` — apply exactly that plan: `let installed = install::apply(&plan)?;`.
- `uninstall::uninstall()` — move a listed mod to trash: `uninstall::uninstall(mods, path)?;`.
- `verify::verify()` — check one inventory item: `let report = verify::verify(&item);`.
- `verify::duplicates()` — find duplicate names: `let duplicates = verify::duplicates(mods);`.
- `paints::list()` — list paints owned by an item: `let paints = paints::list(mods, "KTM");`.
- `paints::install()` — copy a public paint file: `paints::install(file, paint_folder)?;`.
- `paints::read()` — read a paint as bytes: `let bytes = paints::read(&paint)?;`.
- `paints::write()` — write paint bytes safely: `paints::write(folder, "Red", bytes)?;`.
- `paints::remove()` — move a paint to trash: `paints::remove(&paint)?;`.
- `presets::list()` — read the plain preset store: `let presets = presets::list(data_dir);`.
- `presets::read()` — find a named preset: `let preset = presets::read(data_dir, "Race");`.
- `presets::write()` — replace a named preset: `presets::write(data_dir, preset)?;`.
- Game plugins have their own package: see [plugins.md](plugins.md). `mods::list` with `Kind::Plugins` only lists the plugin files.

All public data structures serialize with camel-case field names.

## CLI

The umbrella crate includes `mxb`. Every command accepts `--json`:

```text
mxb mods folders --json
mxb mods list [--kind K] --json
mxb mods identify <path> --json
mxb mods plan <src> --json
mxb mods install <src> --json
mxb mods uninstall <mod-path> --json
mxb mods verify [<mod-path>] --json
mxb mods duplicates --json
mxb mods paints <item> --json
```

Commands that need a game use the first discovered install. `plan` is read-only; `install`
applies the displayed plan.

## TypeScript

`@mxb/core` is a typed Node wrapper around the JSON CLI. Set `MXB_BIN` to an explicit
binary or leave it unset to use `mxb` on `PATH`.

```ts
import { findInstalls, listMods, plan } from "@mxb/core";
const games = await findInstalls();
const bikes = await listMods("bikes");
const preview = await plan("./My Bike");
```

## Boundaries

The library has no network client, does not interpret protected packages, has no app-shell
dependency, and never permanently deletes user files. Removal always uses the operating
system trash.

## Versioning

The API remains `0.x` until it settles. During `0.x`, a breaking change increments the minor
version; compatible additions and fixes increment the patch version.
