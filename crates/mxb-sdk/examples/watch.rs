use mxb_sdk::game::{Client, Message, StreamOptions};

#[tokio::main]
async fn main() {
    let client = Client::new(std::env::var("MXB_GAME_TOKEN").expect("set MXB_GAME_TOKEN"));
    let mut stream = client.stream(StreamOptions::default());
    while let Some(message) = stream.next().await {
        match message {
            Message::State(state) => println!("frame {}", state.frame),
            Message::Event(event) => println!("event {}", event.seq),
            Message::Disconnected(error) => eprintln!("{error}"),
            _ => {}
        }
    }
}

