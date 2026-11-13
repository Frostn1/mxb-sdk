use mxb_game::types::EventKind;
use mxb_game::{format_lap, Client, Message, StreamOptions};
use serde_json::json;
use std::time::Duration;

async fn post(client: &reqwest::Client, webhook: &str, content: String) -> Result<(), Box<dyn std::error::Error>> {
    let body = json!({ "content": content });
    let send = || client.post(webhook).json(&body).send();
    let mut response = send().await?;
    if response.status().as_u16() == 429 {
        let retry = response.json::<serde_json::Value>().await?.get("retry_after").and_then(|v| v.as_f64()).unwrap_or(1.0);
        tokio::time::sleep(Duration::from_secs_f64(retry.max(0.0))).await;
        response = send().await?;
    }
    if !response.status().is_success() {
        eprintln!("Discord returned HTTP {}", response.status());
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("MXB_GAME_TOKEN").expect("set MXB_GAME_TOKEN");
    let webhook = std::env::var("DISCORD_WEBHOOK_URL").expect("set DISCORD_WEBHOOK_URL");
    let discord = webhook.starts_with("https://discord.com/api/webhooks/")
        || webhook.starts_with("https://discordapp.com/api/webhooks/");
    if !discord && std::env::var("MXB_TEST_WEBHOOK_ALLOW_LOCAL").ok().as_deref() != Some("1") {
        return Err("DISCORD_WEBHOOK_URL must be a Discord webhook".into());
    }
    let url = std::env::var("MXB_GAME_URL").unwrap_or_else(|_| "http://127.0.0.1:47820".into());
    let game = Client::builder().url(url).token(token).build();
    let http = reqwest::Client::new();
    let mut stream = game.stream(StreamOptions::default());

    while let Some(message) = stream.next().await {
        if let Message::Event(event) = message {
            match event.kind {
                EventKind::Lap { lap, time_ms, valid: true, best: true, .. } if event.local == Some(true) => {
                    post(&http, &webhook, format!("Personal best: lap {lap} — {}", format_lap(time_ms))).await?;
                }
                EventKind::SessionEnd { .. } => {
                    let state = game.state().await?;
                    let rows = state.riders.iter().take(10).map(|r| {
                        format!("{}. {} — {}", r.place.map_or_else(|| "-".into(), |p| p.to_string()), r.name,
                            r.best_lap_ms.map_or_else(|| "-:--.---".into(), format_lap))
                    }).collect::<Vec<_>>().join("\n");
                    let track = state.session.as_ref().map(|s| s.track_name.as_str()).unwrap_or("session");
                    post(&http, &webhook, format!("Final timing — {track}\n{rows}")).await?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
