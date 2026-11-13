//! Umbrella for the public MX Bikes developer crates.
//!
//! | Feature | Crate | Default |
//! | --- | --- | --- |
//! | `core` | [`core`] filesystem and mod tools | yes |
//! | `game` | [`game`] live Game SDK client | yes |
//! | `extensions` | [`extensions`] extension kit | no |
//! | `plugins` | [`plugins`] game-plugin manifest reader | no |
//! | `mcp` | [`mcp`] stdio bridge | no |
//!
//! `full` enables every member. Disabling default features keeps applications in control of
//! their dependency graph.

/// Public headless MX Bikes filesystem and mod tools.
#[cfg(feature = "core")]
pub use mxb_core as core;
/// Async live-game client and generated public schema.
#[cfg(feature = "game")]
pub use mxb_game as game;
/// Extension manifests, validation, packing, signing, and keys.
#[cfg(feature = "extensions")]
pub use mxb_extensions as extensions;
/// MX Bikes game-plugin manifest contract and reader.
#[cfg(feature = "plugins")]
pub use mxb_plugins as plugins;
/// Stdio-to-HTTP MCP bridge.
#[cfg(feature = "mcp")]
pub use mxb_mcp as mcp;

