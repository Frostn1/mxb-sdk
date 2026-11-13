use mxb_mcp::{bridge, Config};
use std::io::{Read, Write};
use std::net::TcpListener;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt, BufReader};

fn server(status: &str, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let status = status.to_string();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 4096];
        let read = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.contains("Authorization: Bearer secret") || request.contains("authorization: Bearer secret"));
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    format!("http://{address}")
}

async fn run(input: &str, url: String) -> String {
    let (mut input_writer, input_reader) = duplex(4096);
    input_writer.write_all(input.as_bytes()).await.unwrap();
    input_writer.shutdown().await.unwrap();
    let (output_writer, mut output_reader) = duplex(4096);
    bridge(BufReader::new(input_reader), output_writer, Config { url, token: "secret".into() })
        .await
        .unwrap();
    let mut output = String::new();
    output_reader.read_to_string(&mut output).await.unwrap();
    output
}

#[tokio::test]
async fn forwards_a_request_and_returns_the_http_json() {
    let output = run(
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n",
        server("200 OK", r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[{"name":"get_game_state"},{"name":"change_game"}]}}"#),
    )
    .await;
    assert!(output.contains("\"result\""));
    assert!(output.contains("get_game_state"));
    assert!(!output.contains("change_game"));
}

#[tokio::test]
async fn refuses_a_non_read_only_tool_without_http() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let output = run(
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"change_game\"}}\n",
        format!("http://{}", listener.local_addr().unwrap()),
    )
    .await;
    assert!(output.contains("-32601"));
    assert!(output.contains("tool not available: change_game (the SDK is read-only)"));
    assert!(listener.accept().is_err());
}

#[tokio::test]
async fn a_202_notification_has_no_output() {
    let output = run(
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
        server("202 Accepted", ""),
    )
    .await;
    assert!(output.is_empty());
}

#[tokio::test]
async fn malformed_input_returns_a_parse_error() {
    let output = run("not json\n", "http://127.0.0.1:1".into()).await;
    assert!(output.contains("-32700"));
}


#[tokio::test]
async fn refuses_a_batch_without_http() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let output = run(
        "[{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{\"name\":\"change_game\"}}]\n",
        format!("http://{}", listener.local_addr().unwrap()),
    )
    .await;
    assert!(output.contains("-32600"));
    assert!(listener.accept().is_err());
}
