use mxb_extensions::{keygen, pack, read_folder, validate_manifest, verify, Manifest};
use std::fs;
use std::path::PathBuf;

fn temp(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mxb-extensions-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_manifest(root: &std::path::Path, permissions: &[&str]) {
    fs::write(
        root.join("extension.json"),
        serde_json::to_vec(&serde_json::json!({
            "manifestVersion": 1,
            "id": "author.sample",
            "name": "Sample",
            "version": "1.0.0",
            "author": "Author",
            "permissions": permissions,
            "contributes": {}
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
fn validates_good_and_bad_manifests() {
    let root = temp("manifest");
    write_manifest(&root, &["mods.read"]);
    let files = read_folder(&root).unwrap();
    let mut manifest: Manifest = serde_json::from_slice(&files["extension.json"]).unwrap();
    assert!(validate_manifest(&manifest, &files).is_ok());
    manifest.id = "Bad Id".into();
    assert!(validate_manifest(&manifest, &files).is_err());
}

#[test]
fn refuses_unknown_permissions() {
    let root = temp("permission");
    write_manifest(&root, &["files.everything"]);
    let files = read_folder(&root).unwrap();
    let manifest: Manifest = serde_json::from_slice(&files["extension.json"]).unwrap();
    assert!(validate_manifest(&manifest, &files).unwrap_err().to_string().contains("unknown permission"));
}

#[test]
fn signed_pack_round_trip() {
    let root = temp("roundtrip");
    write_manifest(&root, &[]);
    fs::write(root.join("view.html"), "<p>hello</p>").unwrap();
    let key = root.join("author.pem");
    keygen(&key).unwrap();
    let output = root.join("sample.mxbext");
    pack(&root, &output, Some(&key)).unwrap();
    let package = verify(&output, "0.1.0").unwrap();
    assert_eq!(package.manifest.id, "author.sample");
    assert!(package.key.is_some());
}

