use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn souk_cmd() -> assert_cmd::Command {
    cargo_bin_cmd!("souk")
}

/// Test the full lifecycle: init -> add -> validate -> update -> remove
#[test]
fn full_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let tmp_path = tmp.path().to_str().unwrap();

    // 1. Init marketplace
    souk_cmd()
        .args(["init", "--path", tmp_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("Marketplace initialized"));

    // Verify files were created
    assert!(tmp
        .path()
        .join(".claude-plugin")
        .join("marketplace.json")
        .exists());
    assert!(tmp.path().join("plugins").is_dir());

    // 2. Create a plugin on disk
    let plugin_dir = tmp.path().join("plugins").join("test-plugin");
    let claude_dir = plugin_dir.join(".claude-plugin");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("plugin.json"),
        r#"{"name": "test-plugin", "version": "1.0.0", "description": "A test plugin"}"#,
    )
    .unwrap();

    // 3. Add plugin
    let mp_path = tmp.path().join(".claude-plugin").join("marketplace.json");
    souk_cmd()
        .args([
            "add",
            plugin_dir.to_str().unwrap(),
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // 4. Validate plugin
    souk_cmd()
        .args(["validate", "plugin", plugin_dir.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Plugin validated"));

    // 5. Validate marketplace
    souk_cmd()
        .args([
            "validate",
            "marketplace",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Marketplace validation passed"));

    // 6. Update plugin with patch bump
    souk_cmd()
        .args([
            "update",
            "test-plugin",
            "--patch",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Verify version was bumped in plugin.json
    let plugin_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(claude_dir.join("plugin.json")).unwrap()).unwrap();
    assert_eq!(plugin_json["version"], "1.0.1");

    // 7. Remove plugin
    souk_cmd()
        .args([
            "remove",
            "test-plugin",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Verify marketplace is now empty
    let mp_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mp_path).unwrap()).unwrap();
    assert_eq!(mp_json["plugins"].as_array().unwrap().len(), 0);
}

#[test]
fn init_already_exists() {
    let tmp = TempDir::new().unwrap();
    let tmp_path = tmp.path().to_str().unwrap();

    // First init
    souk_cmd()
        .args(["init", "--path", tmp_path])
        .assert()
        .success();

    // Second init should fail
    souk_cmd()
        .args(["init", "--path", tmp_path])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn json_output_mode() {
    let tmp = TempDir::new().unwrap();
    let tmp_path = tmp.path().to_str().unwrap();

    souk_cmd()
        .args(["init", "--path", tmp_path, "--json"])
        .assert()
        .success();

    // Create and add a plugin
    let plugin_dir = tmp.path().join("plugins").join("json-test");
    let claude_dir = plugin_dir.join(".claude-plugin");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("plugin.json"),
        r#"{"name": "json-test", "version": "1.0.0", "description": "Test"}"#,
    )
    .unwrap();

    // Validate with JSON output
    let output = souk_cmd()
        .args(["validate", "plugin", plugin_dir.to_str().unwrap(), "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(parsed["results"].is_array());
}
