# mxb-core

`mxb-core` is the public, headless Rust library behind MXB App's filesystem features.
It finds game installs, inventories and identifies mods, plans safe installs, moves known
mods to the operating system trash, verifies content, and manages paints and presets.

```toml
mxb-core = { path = "crates/mxb-core" }
```

The crate has no Tauri or network dependency. It does not inspect protected packages and it
never permanently deletes user files. See `docs/mxbikes-core.md` for the API and CLI.
