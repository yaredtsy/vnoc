//! One release, one version: the core, every driver in `drivers/` (found, not listed: docs/23 §5) and the extension say the same one.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    dunce::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// `version = "x"` of the `[package]` table of a Cargo.toml.
fn cargo_version(path: &std::path::Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let package = text
        .split("\n[")
        .find(|t| t.starts_with("package]"))
        .unwrap_or(&text);
    package
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|v| v.split('"').next())
        .unwrap_or_else(|| panic!("{}: no package version", path.display()))
        .to_string()
}

#[test]
fn every_driver_carries_the_release_version() {
    let root = repo_root();
    let release = env!("CARGO_PKG_VERSION");
    let mut seen = 0;
    for entry in std::fs::read_dir(root.join("drivers")).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let manifest = dir.join("Cargo.toml");
        assert_eq!(
            cargo_version(&manifest),
            release,
            "drivers/{name}/Cargo.toml is not at the release version {release}"
        );
        seen += 1;
    }
    assert!(seen > 0, "drivers/ holds no driver");
}

#[test]
fn the_extension_carries_the_release_version() {
    let text = std::fs::read_to_string(repo_root().join("extension/package.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        json["version"],
        env!("CARGO_PKG_VERSION"),
        "extension/package.json is not at the release version"
    );
}
