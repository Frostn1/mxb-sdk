//! Async client and generated public schemas for MXB App's live Game SDK.
//!
//! [`Client`] covers the authenticated HTTP API. [`Client::stream`] runs the blocking
//! WebSocket transport on a dedicated thread and delivers typed [`Message`] values through
//! Tokio, so applications do not need a second WebSocket dependency.

mod client;
pub mod types;

/// Primary client, stream, configuration, and error types.
pub use client::{Builder, Client, Error, Message, Stream, StreamOptions};

/// Formats milliseconds as a lap time (`95123` becomes `1:35.123`).
pub fn format_lap(ms: i64) -> String {
    if ms <= 0 {
        return "-:--.---".into();
    }
    format!("{}:{:02}.{:03}", ms / 60_000, (ms / 1_000) % 60, ms % 1_000)
}

#[cfg(test)]
mod tests {
    use super::format_lap;

    #[test]
    fn formats_lap_times() {
        assert_eq!(format_lap(95_123), "1:35.123");
        assert_eq!(format_lap(5_007), "0:05.007");
        assert_eq!(format_lap(0), "-:--.---");
        assert_eq!(format_lap(-1), "-:--.---");
    }
}
