//! Async client and generated public schemas for MXB App's live Game SDK.
//!
//! [`Client`] covers the authenticated HTTP API. [`Client::stream`] runs the blocking
//! WebSocket transport on a dedicated thread and delivers typed [`Message`] values through
//! Tokio, so applications do not need a second WebSocket dependency.

mod client;
pub mod types;

/// Primary client, stream, configuration, and error types.
pub use client::{ActionReceipt, Builder, Client, Error, Message, Stream, StreamOptions};
