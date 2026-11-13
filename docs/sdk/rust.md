# Rust packages

The Rust crates can be selected separately or through `mxb-sdk`.

| Crate | Umbrella feature | Default | Purpose |
|---|---|---:|---|
| `mxb-core` | `core` | yes | Folders, mods, identify, install, uninstall, verify, paints, presets |
| `mxb-game` | `game` | yes | Live game client and public schemas |
| `mxb-extensions` | `extensions` | no | Extension validation, packing, signing, and keys |
| `mxb-plugins` | `plugins` | no | MX Bikes game plugins: the `.mxbplugin.json` manifest, listing, turning on and off |
| `mxb-mcp` | `mcp` | no | MCP bridge support |

```toml
[dependencies]
mxb-sdk = { path = "../mxb-app/crates/mxb-sdk", features = ["core", "game"] }
```

The default feature set is `core` plus `game`, so the short form is equivalent:

```toml
mxb-sdk = { path = "../mxb-app/crates/mxb-sdk" }
```

```rust
use mxb_sdk::core::folders;

fn main() {
    for install in folders::find_installs() {
        println!("{}", install.mods_dir.display());
    }
}
```

Use `default-features = false` for a smaller combination.
