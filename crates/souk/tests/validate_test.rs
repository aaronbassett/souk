use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests")
        .join("fixtures")
}

fn souk_cmd() -> assert_cmd::Command {
    cargo_bin_cmd!("souk")
}

#[test]
fn validate_valid_plugin_by_path() {
    let plugin = fixtures_dir()
        .join("valid-marketplace")
        .join("plugins")
        .join("good-plugin");

    souk_cmd()
        .args(["validate", "plugin", plugin.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Plugin validated: good-plugin"));
}

#[test]
fn validate_invalid_plugin_by_path() {
    let plugin = fixtures_dir().join("invalid-plugin");

    souk_cmd()
        .args(["validate", "plugin", plugin.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ERROR:"));
}

#[test]
fn validate_marketplace_valid() {
    let mp = fixtures_dir()
        .join("valid-marketplace")
        .join(".claude-plugin")
        .join("marketplace.json");

    souk_cmd()
        .args([
            "validate",
            "marketplace",
            "--marketplace",
            mp.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Marketplace validation passed"));
}

#[test]
fn validate_plugin_json_output() {
    let plugin = fixtures_dir()
        .join("valid-marketplace")
        .join("plugins")
        .join("good-plugin");

    let output = souk_cmd()
        .args(["validate", "plugin", plugin.to_str().unwrap(), "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["results"].is_array());
}

#[test]
fn validate_nonexistent_plugin() {
    souk_cmd()
        .args(["validate", "plugin", "/tmp/nonexistent-souk-test-xyz"])
        .assert()
        .failure();
}
