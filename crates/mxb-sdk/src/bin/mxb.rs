use serde::Serialize;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(error) = run() {
        eprintln!("mxb: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json = take(&mut args, "--json");
    let group = args.first().map(String::as_str).unwrap_or("help");
    match group {
        "mods" => {
            #[cfg(feature = "core")]
            return mods(&args[1..], json);
            #[cfg(not(feature = "core"))]
            return missing("core");
        }
        "game" => {
            #[cfg(feature = "game")]
            return game(&args[1..], json);
            #[cfg(not(feature = "game"))]
            return missing("game");
        }
        "ext" => {
            #[cfg(feature = "extensions")]
            return extensions(&args[1..], json);
            #[cfg(not(feature = "extensions"))]
            return missing("extensions");
        }
        "plugins" => {
            #[cfg(feature = "plugins")]
            return plugins(&args[1..], json);
            #[cfg(not(feature = "plugins"))]
            return missing("plugins");
        }
        "mcp" => {
            #[cfg(feature = "mcp")]
            return mcp(&args[1..]);
            #[cfg(not(feature = "mcp"))]
            return missing("mcp");
        }
        _ => Err("usage: mxb mods|game|ext|plugins|mcp ...".into()),
    }
}

fn missing(feature: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err(format!("this command requires the '{feature}' feature").into())
}

#[cfg(feature = "core")]
fn mods(args: &[String], json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use mxb_sdk::core::{self, mods::Kind};
    let command = args.first().map(String::as_str).unwrap_or("help");
    let installs = core::folders::find_installs();
    let first = installs.first();
    match command {
        "folders" => print(&installs, json),
        "list" => {
            let kind = value(args, "--kind")
                .map(|value| Kind::parse(value).ok_or_else(|| format!("unknown kind {value}")))
                .transpose()?;
            print(&core::mods::list(mods_dir(first)?, kind), json)
        }
        "identify" => print(&core::identify::identify(required(args, 1, "path")?)?, json),
        "plan" => print(&core::install::plan(required(args, 1, "source")?, mods_dir(first)?, None)?, json),
        "install" => {
            let plan = core::install::plan(required(args, 1, "source")?, mods_dir(first)?, None)?;
            print(&core::install::apply(&plan)?, json)
        }
        "uninstall" => {
            let path = PathBuf::from(required(args, 1, "path")?);
            let root = mods_root(&path)
                .or_else(|| first.map(|install| install.mods_dir.clone()))
                .ok_or("could not determine the mods folder")?;
            print(&core::uninstall::uninstall(root, path)?, json)
        }
        "verify" => {
            if let Some(path) = args.get(1) {
                let path = PathBuf::from(path);
                let item = core::mods::list(mods_dir(first)?, None)
                    .into_iter()
                    .find(|item| item.path == path)
                    .ok_or("path is not a listed mod")?;
                print(&core::verify::verify(&item), json)
            } else {
                let reports: Vec<_> = core::mods::list(mods_dir(first)?, None)
                    .into_iter()
                    .map(|item| (item.path.clone(), core::verify::verify(&item)))
                    .collect();
                print(&reports, json)
            }
        }
        "duplicates" => print(&core::verify::duplicates(mods_dir(first)?), json),
        "paints" => print(&core::paints::list(mods_dir(first)?, required(args, 1, "item")?), json),
        _ => return Err("usage: mxb mods folders|list|identify|plan|install|uninstall|verify|duplicates|paints".into()),
    }
    Ok(())
}

#[cfg(feature = "game")]
fn game(args: &[String], json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use mxb_sdk::game::{types::ContentList, Client, Message, StreamOptions};
    let command = args.first().map(String::as_str).unwrap_or("help");
    let token = std::env::var("MXB_GAME_TOKEN").map_err(|_| "set MXB_GAME_TOKEN")?;
    let client = Client::builder()
        .token(token)
        .url(std::env::var("MXB_GAME_URL").unwrap_or_else(|_| "http://127.0.0.1:47820".into()))
        .build();
    runtime()?.block_on(async move {
        match command {
            "hello" => print(&client.hello().await?, json),
            "state" => print(&client.state().await?, json),
            "laps" => print(&client.laps().await?, json),
            "reload" => print(&client.reload_mods().await?, json),
            "refresh" => {
                let list = match required(args, 1, "list")? {
                    "tracks" => ContentList::Tracks,
                    "bikes" => ContentList::Bikes,
                    "gear" => ContentList::Gear,
                    "paints" => ContentList::Paints,
                    "all" => ContentList::All,
                    _ => return Err("list must be tracks, bikes, gear, paints, or all".into()),
                };
                print(&client.refresh(list).await?, json)
            }
            "watch" => {
                let mut stream = client.stream(StreamOptions::default());
                while let Some(message) = stream.next().await {
                    match message {
                        Message::Hello(value) => print(&value, true),
                        Message::State(value) => print(&value, true),
                        Message::Event(value) => print(&value, true),
                        Message::Disconnected(detail) => eprintln!("disconnected: {detail}"),
                        Message::Reconnected => eprintln!("reconnected"),
                    }
                }
            }
            _ => return Err("usage: mxb game hello|state|watch|laps|reload|refresh <list>".into()),
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}

#[cfg(feature = "extensions")]
fn extensions(args: &[String], json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use mxb_sdk::extensions as ext;
    match args.first().map(String::as_str).unwrap_or("help") {
        "validate" => {
            let root = Path::new(required(args, 1, "directory")?);
            let files = ext::read_folder(root)?;
            let manifest: ext::Manifest = serde_json::from_slice(files.get("extension.json").ok_or("extension.json is missing")?)?;
            ext::validate_manifest(&manifest, &files)?;
            print(&serde_json::json!({ "valid": true, "id": manifest.id }), json)
        }
        "pack" => {
            let root = Path::new(required(args, 1, "directory")?);
            let key = value(args, "--key").map(Path::new);
            let files = ext::read_folder(root)?;
            let manifest: ext::Manifest = serde_json::from_slice(files.get("extension.json").ok_or("extension.json is missing")?)?;
            let default = format!("{}-{}.mxbext", manifest.id, manifest.version);
            let output = value(args, "--out").unwrap_or(&default);
            let result = ext::pack(root, Path::new(output), key)?;
            print(&serde_json::json!({ "path": result.path, "files": result.files, "bytes": result.bytes, "publicKey": result.public_key }), json)
        }
        "keygen" => {
            let path = Path::new(required(args, 1, "key path")?);
            let public_key = ext::keygen(path)?;
            print(&serde_json::json!({ "path": path, "publicKey": public_key }), json)
        }
        _ => return Err("usage: mxb ext validate <dir>|pack <dir> [--key k.pem]|keygen <k.pem>".into()),
    }
    Ok(())
}

#[cfg(feature = "plugins")]
fn plugins(args: &[String], json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use mxb_sdk::plugins as game_plugins;
    match args.first().map(String::as_str).unwrap_or("help") {
        "list" => {
            let game_dir = if let Some(folder) = args.get(1) {
                Path::new(folder).parent().ok_or("invalid plugins folder")?.to_path_buf()
            } else {
                mxb_core::folders::find_installs()
                    .into_iter()
                    .next()
                    .map(|install| install.install_dir)
                    .ok_or("no game install was found")?
            };
            print(&game_plugins::list(&game_dir), json)
        }
        "inspect" => {
            let bytes = std::fs::read(required(args, 1, "manifest file")?)?;
            let manifest: game_plugins::Manifest = serde_json::from_slice(&bytes)?;
            game_plugins::validate_manifest(&manifest)?;
            print(&manifest, json)
        }
        _ => return Err("usage: mxb plugins list [plugins-folder]|inspect <file>".into()),
    }
    Ok(())
}

#[cfg(feature = "mcp")]
fn mcp(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let token = value(args, "--token")
        .map(str::to_string)
        .or_else(|| std::env::var("MXB_GAME_TOKEN").ok())
        .ok_or("set MXB_GAME_TOKEN")?;
    let url = value(args, "--url")
        .map(str::to_string)
        .or_else(|| std::env::var("MXB_GAME_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:47820".into());
    runtime()?.block_on(mxb_sdk::mcp::run(mxb_sdk::mcp::Config { url, token }))?;
    Ok(())
}

#[cfg(any(feature = "game", feature = "mcp"))]
fn runtime() -> Result<tokio::runtime::Runtime, std::io::Error> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build()
}

fn print<T: Serialize + std::fmt::Debug>(value: &T, json: bool) {
    if json { println!("{}", serde_json::to_string(value).unwrap_or_else(|_| "null".into())); }
    else { println!("{value:#?}"); }
}
fn take(args: &mut Vec<String>, value: &str) -> bool {
    args.iter().position(|arg| arg == value).map(|index| args.remove(index)).is_some()
}
fn value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter().position(|arg| arg == key).and_then(|index| args.get(index + 1)).map(String::as_str)
}
fn required<'a>(args: &'a [String], index: usize, name: &str) -> Result<&'a str, Box<dyn std::error::Error>> {
    args.get(index).map(String::as_str).ok_or_else(|| format!("missing {name}").into())
}

#[cfg(feature = "core")]
fn mods_dir(install: Option<&mxb_sdk::core::folders::Install>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    install.map(|install| install.mods_dir.clone()).ok_or_else(|| "no game install was found".into())
}

fn mods_root(path: &Path) -> Option<PathBuf> {
    path.ancestors().find(|ancestor| ancestor.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.eq_ignore_ascii_case("mods"))).map(Path::to_path_buf)
}
