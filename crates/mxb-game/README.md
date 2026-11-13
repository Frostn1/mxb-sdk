# mxb-game

`mxb-game` is the async Rust client and generated public schema for MXB App's local Game SDK.
It supports authenticated read-only HTTP calls and a reconnecting WebSocket stream.

```rust
let client = mxb_game::Client::new(std::env::var("MXB_GAME_TOKEN")?);
let snapshot = client.state().await?;
```

`format_lap(95_123)` returns `1:35.123`. The `lap_logger`, `session_logger`, and `discord_laps`
examples show read-only event handling, CSV output, and Discord timing updates.

