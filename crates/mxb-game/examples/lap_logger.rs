use mxb_game::{format_lap, Client, Message, StreamOptions};

#[tokio::main]
async fn main() {
    let token = std::env::var("MXB_GAME_TOKEN").expect("set MXB_GAME_TOKEN");
    let client = Client::new(token);
    client.connect().await.expect("compatible MXB App");
    let mut stream = client.stream(StreamOptions::default());

    while let Some(message) = stream.next().await {
        if let Message::Event(event) = message {
            if let mxb_game::types::EventKind::Lap { lap, time_ms, .. } = event.kind {
                println!("lap {lap}: {}", format_lap(time_ms));
            }
        }
    }
}

