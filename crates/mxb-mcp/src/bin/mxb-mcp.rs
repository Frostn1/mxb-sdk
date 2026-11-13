use mxb_mcp::Config;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("mxb-mcp: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };
    let token = value("--token")
        .or_else(|| std::env::var("MXB_GAME_TOKEN").ok())
        .ok_or("set MXB_GAME_TOKEN (MXB App -> Settings -> Game SDK)")?;
    let url = value("--url")
        .or_else(|| std::env::var("MXB_GAME_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:47820".into());
    mxb_mcp::run(Config { url, token }).await?;
    Ok(())
}

