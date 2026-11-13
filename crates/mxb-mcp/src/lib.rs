//! Line-oriented stdio bridge to MXB App's local HTTP MCP endpoint.

use serde_json::{json, Value};
use std::fmt;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

/// Bridge configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// Base Game SDK URL. `/mcp` is appended by the bridge.
    pub url: String,
    /// Game SDK bearer token.
    pub token: String,
}

impl Config {
    /// Creates configuration using the default local Game SDK URL.
    pub fn new(token: impl Into<String>) -> Self {
        Self { url: "http://127.0.0.1:47820".into(), token: token.into() }
    }

    /// Returns the final HTTP MCP endpoint.
    pub fn endpoint(&self) -> String {
        format!("{}/mcp", self.url.trim_end_matches('/'))
    }
}

/// A bridge-level I/O or HTTP error.
#[derive(Debug)]
pub enum Error {
    /// Reading stdin or writing stdout failed.
    Io(std::io::Error),
    /// Sending a request to MXB App failed outside an individual JSON-RPC request.
    Http(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Http(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Runs the bridge on the process's stdin and stdout until stdin closes.
pub async fn run(config: Config) -> Result<(), Error> {
    bridge(BufReader::new(tokio::io::stdin()), tokio::io::stdout(), config).await
}

/// Bridges JSON-RPC lines from any asynchronous reader to any asynchronous writer.
pub async fn bridge<R, W>(reader: R, mut writer: W, config: Config) -> Result<(), Error>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let endpoint = config.endpoint();
    let client = reqwest::Client::new();
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(_) => {
                write_json(&mut writer, &json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": { "code": -32700, "message": "parse error" }
                }))
                .await?;
                continue;
            }
        };
        let id = message.get("id").cloned();
        let response = client
            .post(&endpoint)
            .bearer_auth(&config.token)
            .header("Accept", "application/json")
            .json(&message)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                if let Some(id) = id {
                    write_json(&mut writer, &json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32000, "message": format!(
                            "MXB App is not reachable at {endpoint}: is the Game SDK local API on? ({error})"
                        ) }
                    }))
                    .await?;
                }
                continue;
            }
        };
        if response.status().as_u16() == 202 {
            continue;
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            if let Some(id) = id {
                let detail: String = body.chars().take(200).collect();
                write_json(&mut writer, &json!({
                    "jsonrpc": "2.0", "id": id,
                    "error": { "code": -32000, "message": format!(
                        "MXB App answered {}: {detail}", status.as_u16()
                    ) }
                }))
                .await?;
            }
        } else if !body.trim().is_empty() {
            writer.write_all(body.trim().as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }
    }
    Ok(())
}

async fn write_json<W: AsyncWrite + Unpin>(writer: &mut W, value: &Value) -> Result<(), Error> {
    writer.write_all(value.to_string().as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

