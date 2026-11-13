use mxb_plugins::{list, manifest_name, plugin_files, read_manifest_checked, set_enabled};
use std::fs;
use std::path::PathBuf;

fn temp(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mxb-plugins-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(path.join("plugins")).unwrap();
    path
}

#[test]
fn parses_manifest_and_enforces_limits() {
    let game = temp("manifest");
    let plugins = game.join("plugins");
    fs::write(plugins.join("alpha.dlo"), b"x").unwrap();
    fs::write(plugins.join(manifest_name("alpha.dlo")), r#"{"name":"Alpha","category":"hud"}"#).unwrap();
    assert_eq!(read_manifest_checked(&plugins, "alpha.dlo").unwrap().unwrap().name.as_deref(), Some("Alpha"));
    let too_many = vec!["x"; 65];
    fs::write(plugins.join("alpha.mxbplugin.json"), serde_json::to_vec(&serde_json::json!({ "files": too_many })).unwrap()).unwrap();
    assert!(read_manifest_checked(&plugins, "alpha.dlo").is_err());
}

#[test]
fn groups_files_lists_plugins_and_never_lists_stock() {
    let game = temp("files");
    let plugins = game.join("plugins");
    fs::write(plugins.join("alpha.dlo"), b"x").unwrap();
    fs::create_dir(plugins.join("alpha_data")).unwrap();
    fs::write(plugins.join("proxy64.dlo"), b"stock").unwrap();
    let files = plugin_files(&plugins, "alpha").unwrap();
    assert!(files.iter().any(|path| path.ends_with("alpha_data")));
    let found = list(&game);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, "alpha");
}

#[test]
fn enable_disable_refuses_collisions() {
    let game = temp("moves");
    fs::write(game.join("plugins/alpha.dlo"), b"active").unwrap();
    fs::create_dir_all(game.join("mxbapp_disabled/plugins")).unwrap();
    fs::write(game.join("mxbapp_disabled/plugins/alpha.dlo"), b"disabled").unwrap();
    assert!(set_enabled(&game, "alpha", false, false).is_err());
    assert!(set_enabled(&game, "alpha", false, true).is_err());
    assert_eq!(fs::read(game.join("plugins/alpha.dlo")).unwrap(), b"active");
}

