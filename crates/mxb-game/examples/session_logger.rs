use mxb_game::types::EventKind;
use mxb_game::{Client, Message, StreamOptions};
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn new_csv(folder: &PathBuf) -> std::io::Result<(PathBuf, File)> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let path = folder.join(format!("session-{stamp}.csv"));
    let mut file = File::create(&path)?;
    writeln!(file, "lap,timeMs,valid,best,splits,topSpeedKmh,avgSpeedKmh,track,bike,at")?;
    Ok((path, file))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("MXB_GAME_TOKEN").expect("set MXB_GAME_TOKEN");
    let url = std::env::var("MXB_GAME_URL").unwrap_or_else(|_| "http://127.0.0.1:47820".into());
    let folder = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "mxb-sessions".into()));
    create_dir_all(&folder)?;
    let client = Client::builder().url(url).token(token).build();
    let mut state = client.state().await?;
    let (mut path, mut file) = new_csv(&folder)?;
    println!("writing {}", path.display());
    let mut stream = client.stream(StreamOptions::default());

    while let Some(message) = stream.next().await {
        match message {
            Message::State(next) => state = next,
            Message::Event(event) => match event.kind {
                EventKind::EventStart { .. } => {
                    (path, file) = new_csv(&folder)?;
                    println!("writing {}", path.display());
                }
                EventKind::Lap { lap, time_ms, valid, best, splits_ms, top_speed_kmh, avg_speed_kmh, .. }
                    if event.local == Some(true) => {
                    let splits = splits_ms.unwrap_or_default().iter().map(i64::to_string).collect::<Vec<_>>().join(";");
                    let track = state.session.as_ref().map(|s| s.track_name.as_str()).unwrap_or("");
                    let bike = state.local.as_ref().map(|l| l.bike.name.as_str()).unwrap_or("");
                    writeln!(file, "{lap},{time_ms},{valid},{best},\"{splits}\",{},{},\"{track}\",\"{bike}\",{}",
                        top_speed_kmh.unwrap_or_default(), avg_speed_kmh.unwrap_or_default(), event.at)?;
                    file.flush()?;
                }
                EventKind::SessionEnd { .. } => {
                    let final_state = client.state().await?;
                    let stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("session");
                    let result_path = path.with_file_name(format!("{stem}-results.json"));
                    let result = serde_json::json!({ "session": final_state.session, "results": final_state.riders });
                    let mut result_file = OpenOptions::new().create(true).truncate(true).write(true).open(result_path)?;
                    serde_json::to_writer_pretty(&mut result_file, &result)?;
                    writeln!(result_file)?;
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}
