use mxb_game::{Client, Error, Message, StreamOptions};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn http_server(responses: Vec<(&'static str, &'static str)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    std::thread::spawn(move || {
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = [0u8; 8192];
            let read = stream.read(&mut bytes).unwrap();
            seen.lock().unwrap().push(String::from_utf8_lossy(&bytes[..read]).into_owned());
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    (format!("http://{address}"), requests)
}

const HELLO: &str = r#"{"schema":1,"sdk":"1.1.0","app":"0.21.0","capabilities":[],"frostmod":null,"deprecated":[]}"#;

#[tokio::test]
async fn sends_the_bearer_token() {
    let (url, requests) = http_server(vec![("200 OK", HELLO)]);
    Client::builder().url(url).token("secret").build().hello().await.unwrap();
    assert!(requests.lock().unwrap()[0].to_ascii_lowercase().contains("authorization: bearer secret"));
}

#[tokio::test]
async fn refuses_an_incompatible_schema() {
    let (url, _) = http_server(vec![(
        "200 OK",
        r#"{"schema":2,"sdk":"2.0.0","app":"0.21.0","capabilities":[],"frostmod":null,"deprecated":[]}"#,
    )]);
    let error = Client::builder().url(url).token("secret").build().connect().await.unwrap_err();
    assert!(matches!(error, Error::Incompatible { server: 2, client: 1 }));
}

#[tokio::test]
async fn retries_one_429_action() {
    let (url, requests) = http_server(vec![
        ("429 Too Many Requests", r#"{"error":"slow down"}"#),
        ("202 Accepted", r#"{"id":"a1","action":"reload_mods","state":"sent"}"#),
    ]);
    let receipt = Client::builder().url(url).token("secret").build().reload_mods().await.unwrap();
    assert_eq!(receipt.id, "a1");
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn waits_for_an_action_to_finish() {
    let accepted = r#"{"id":"a1","action":"reload_mods","state":"accepted","detail":"","lists":[],"at":1}"#;
    let done = r#"{"id":"a1","action":"reload_mods","state":"done","detail":"","lists":[],"at":2}"#;
    let (url, _) = http_server(vec![("200 OK", accepted), ("200 OK", done)]);
    let status = Client::builder()
        .url(url)
        .token("secret")
        .build()
        .wait_for_action("a1", Duration::from_secs(2))
        .await
        .unwrap();
    assert!(matches!(status.state, mxb_game::types::ActionState::Done));
}

#[tokio::test]
async fn stream_delivers_hello_state_event_in_order() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        socket.send(tungstenite::Message::Text(format!(r#"{{"type":"hello","schema":1,"sdk":"1.1.0","app":"0.21.0","capabilities":[],"frostmod":null,"deprecated":[],"rate":20}}"#).into())).unwrap();
        socket.send(tungstenite::Message::Text(r#"{"type":"state","data":{"schema":1,"frame":7,"at":1,"source":"live","online":false,"session":null,"local":null,"riders":[],"frostmod":null}}"#.into())).unwrap();
        socket.send(tungstenite::Message::Text(r#"{"type":"event","data":{"seq":8,"at":2,"type":"on_track"}}"#.into())).unwrap();
    });
    let client = Client::builder().url(format!("http://{address}")).token("secret").build();
    let mut stream = client.stream(StreamOptions { reconnect: false, ..Default::default() });
    let first = tokio::time::timeout(Duration::from_secs(2), stream.next()).await.unwrap().unwrap();
    let second = stream.next().await.unwrap();
    let third = stream.next().await.unwrap();
    assert!(matches!(first, Message::Hello(_)));
    assert!(matches!(second, Message::State(state) if state.frame == 7));
    assert!(matches!(third, Message::Event(event) if event.seq == 8));
}

