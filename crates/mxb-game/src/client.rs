use crate::types::{Event, Hello, LapRecord, Snapshot, SCHEMA_VERSION};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

const DEFAULT_URL: &str = "http://127.0.0.1:47820";

/// An error returned by the Game SDK client.
#[derive(Debug)]
pub enum Error {
    /// An HTTP request failed or returned a non-success status.
    Http(String),
    /// The server and client use different schema major versions.
    Incompatible { server: i64, client: u32 },
    /// A local I/O operation failed.
    Io(std::io::Error),
    /// A JSON response or stream message was malformed.
    Json(serde_json::Error),
    /// The WebSocket connection failed.
    WebSocket(tungstenite::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(message) => f.write_str(message),
            Self::Incompatible { server, client } => write!(
                f,
                "incompatible Game SDK schema {server}; this client supports schema {client}"
            ),
            Self::Io(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::WebSocket(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<tungstenite::Error> for Error {
    fn from(error: tungstenite::Error) -> Self {
        Self::WebSocket(error)
    }
}

/// Builds a [`Client`] while keeping the default local endpoint explicit.
#[derive(Clone, Debug)]
pub struct Builder {
    url: String,
    token: String,
}

impl Default for Builder {
    fn default() -> Self {
        Self { url: DEFAULT_URL.into(), token: String::new() }
    }
}

impl Builder {
    /// Sets the base HTTP URL, without a required trailing slash.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Sets the token copied from MXB App's Game SDK settings.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = token.into();
        self
    }

    /// Creates the client.
    pub fn build(self) -> Client {
        Client::with_url(self.url, self.token)
    }
}

/// An authenticated client for MXB App's local Game SDK endpoint.
#[derive(Clone, Debug)]
pub struct Client {
    url: String,
    token: String,
    http: reqwest::Client,
}

impl Client {
    /// Creates a client using `http://127.0.0.1:47820`.
    pub fn new(token: impl Into<String>) -> Self {
        Self::with_url(DEFAULT_URL, token)
    }

    /// Starts a client builder.
    pub fn builder() -> Builder {
        Builder::default()
    }

    fn with_url(url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            url: url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            http: reqwest::Client::new(),
        }
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, Error> {
        let response = self
            .http
            .get(format!("{}{}", self.url, path))
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|error| Error::Http(error.to_string()))?;
        Self::response(response).await
    }

    async fn response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, Error> {
        let status = response.status();
        let bytes = response.bytes().await.map_err(|error| Error::Http(error.to_string()))?;
        if !status.is_success() {
            let detail = serde_json::from_slice::<ErrorBody>(&bytes)
                .ok()
                .and_then(|body| body.error)
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(Error::Http(detail));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn compatible(hello: Hello) -> Result<Hello, Error> {
        if hello.schema != SCHEMA_VERSION as i64 {
            return Err(Error::Incompatible { server: hello.schema, client: SCHEMA_VERSION });
        }
        Ok(hello)
    }

    /// Returns server versions, enabled capabilities, and integration state.
    pub async fn hello(&self) -> Result<Hello, Error> {
        Self::compatible(self.get("/v1/hello").await?)
    }

    /// Performs the compatibility handshake before other work begins.
    pub async fn connect(&self) -> Result<Hello, Error> {
        self.hello().await
    }

    /// Returns whether the local endpoint is reachable. This endpoint needs no token.
    pub async fn health(&self) -> bool {
        self.http
            .get(format!("{}/v1/health", self.url))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
    }

    /// Returns the current game snapshot.
    pub async fn state(&self) -> Result<Snapshot, Error> {
        self.get("/v1/state").await
    }

    /// Returns events after `since`, oldest first, and the latest sequence number.
    pub async fn events(&self, since: i64) -> Result<(Vec<Event>, i64), Error> {
        let page: EventsPage = self.get(&format!("/v1/events?since={since}")).await?;
        let events = page
            .events
            .into_iter()
            .filter_map(|event| serde_json::from_value(event).ok())
            .collect();
        Ok((events, page.last))
    }

    /// Returns the player's completed laps for the current session.
    pub async fn laps(&self) -> Result<Vec<LapRecord>, Error> {
        Ok(self.get::<LapsPage>("/v1/laps").await?.laps)
    }

    /// Opens a live stream. The connection and reconnect loop run on a standard thread.
    pub fn stream(&self, options: StreamOptions) -> Stream {
        let (tx, rx) = mpsc::channel(128);
        let closed = Arc::new(AtomicBool::new(false));
        let stop = Arc::clone(&closed);
        let url = stream_url(&self.url, &self.token, &options);
        std::thread::spawn(move || stream_thread(url, options.reconnect, stop, tx));
        Stream { rx, closed }
    }
}

/// Options for a live WebSocket stream.
#[derive(Clone, Debug)]
pub struct StreamOptions {
    /// Frames per second, clamped to 1 through 60.
    pub rate: u8,
    /// Whether state snapshots are included.
    pub state: bool,
    /// Whether events are included.
    pub events: bool,
    /// Whether a broken connection is reopened with exponential backoff.
    pub reconnect: bool,
}

impl Default for StreamOptions {
    fn default() -> Self {
        Self { rate: 20, state: true, events: true, reconnect: true }
    }
}

/// A typed message from the live stream.
#[derive(Debug)]
pub enum Message {
    /// The server handshake, always the first message on a connection.
    Hello(Hello),
    /// A live game snapshot.
    State(Snapshot),
    /// A numbered game event.
    Event(Event),
    /// The connection closed or failed.
    Disconnected(String),
    /// A connection was restored after a failure.
    Reconnected,
}

/// Receiver for live [`Message`] values.
pub struct Stream {
    rx: mpsc::Receiver<Message>,
    closed: Arc<AtomicBool>,
}

impl Stream {
    /// Waits for the next stream message, or returns `None` after closure.
    pub async fn next(&mut self) -> Option<Message> {
        self.rx.recv().await
    }

    /// Stops reconnecting and closes this receiver when its worker notices the flag.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: Option<String>,
}

#[derive(Deserialize)]
struct EventsPage {
    events: Vec<serde_json::Value>,
    last: i64,
}

#[derive(Deserialize)]
struct LapsPage {
    laps: Vec<LapRecord>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum WireMessage {
    Hello {
        schema: i64,
        sdk: String,
        app: String,
        capabilities: Vec<crate::types::Capability>,
        deprecated: Vec<crate::types::Deprecation>,
    },
    State { data: Snapshot },
    Event { data: serde_json::Value },
}

fn stream_url(base: &str, token: &str, options: &StreamOptions) -> String {
    let scheme = if base.starts_with("https://") { "wss://" } else { "ws://" };
    let rest = base.split_once("://").map(|(_, rest)| rest).unwrap_or(base);
    let token: String = url::form_urlencoded::byte_serialize(token.as_bytes()).collect();
    let topics = match (options.state, options.events) {
        (true, true) => "state,events",
        (true, false) => "state",
        (false, true) => "events",
        (false, false) => "",
    };
    format!(
        "{scheme}{rest}/v1/stream?token={token}&rate={}&topics={topics}",
        options.rate.clamp(1, 60)
    )
}

fn stream_thread(
    url: String,
    reconnect: bool,
    closed: Arc<AtomicBool>,
    tx: mpsc::Sender<Message>,
) {
    let mut delay = Duration::from_millis(500);
    let mut connected_once = false;
    while !closed.load(Ordering::Relaxed) {
        match tungstenite::connect(url.as_str()) {
            Ok((mut socket, _)) => {
                let restored = connected_once;
                let mut compatible = false;
                loop {
                    if closed.load(Ordering::Relaxed) {
                        let _ = socket.close(None);
                        return;
                    }
                    let text = match socket.read() {
                        Ok(tungstenite::Message::Text(text)) => text.to_string(),
                        Ok(tungstenite::Message::Close(_)) => break,
                        Ok(_) => continue,
                        Err(error) => {
                            let _ = tx.blocking_send(Message::Disconnected(error.to_string()));
                            break;
                        }
                    };
                    let wire: WireMessage = match serde_json::from_str(&text) {
                        Ok(message) => message,
                        Err(error) => {
                            let _ = tx.blocking_send(Message::Disconnected(error.to_string()));
                            break;
                        }
                    };
                    let message = match wire {
                        WireMessage::Hello { schema, sdk, app, capabilities, deprecated } => {
                            if schema != SCHEMA_VERSION as i64 {
                                let error = Error::Incompatible { server: schema, client: SCHEMA_VERSION };
                                let _ = tx.blocking_send(Message::Disconnected(error.to_string()));
                                return;
                            }
                            compatible = true;
                            connected_once = true;
                            delay = Duration::from_millis(500);
                            if restored && tx.blocking_send(Message::Reconnected).is_err() {
                                return;
                            }
                            Message::Hello(Hello { schema, sdk, app, capabilities, deprecated })
                        }
                        WireMessage::State { data } => Message::State(data),
                        WireMessage::Event { data } => match serde_json::from_value(data) {
                            Ok(event) => Message::Event(event),
                            Err(_) => continue,
                        },
                    };
                    if tx.blocking_send(message).is_err() {
                        return;
                    }
                }
                if compatible {
                    let _ = tx.blocking_send(Message::Disconnected("WebSocket closed".into()));
                }
            }
            Err(error) => {
                let _ = tx.blocking_send(Message::Disconnected(error.to_string()));
            }
        }
        if !reconnect || closed.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(delay);
        delay = (delay * 2).min(Duration::from_secs(10));
    }
}

