use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn souk_cmd() -> assert_cmd::Command {
    cargo_bin_cmd!("souk")
}

fn setup_marketplace(tmp: &TempDir, registered: &[&str], on_disk: &[&str]) {
    let claude_dir = tmp.path().join(".claude-plugin");
    fs::create_dir_all(&claude_dir).unwrap();
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    // Create all directories on disk
    for name in on_disk {
        let plugin_dir = plugins_dir.join(name).join(".claude-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            format!(r#"{{"name":"{name}","version":"1.0.0","description":"test"}}"#),
        )
        .unwrap();
    }

    // Register only the specified plugins in marketplace.json
    let entries: Vec<String> = registered
        .iter()
        .map(|name| format!(r#"{{"name":"{name}","source":"{name}"}}"#))
        .collect();
    let mp_json = format!(
        r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{}]}}"#,
        entries.join(",")
    );
    fs::write(claude_dir.join("marketplace.json"), mp_json).unwrap();
}

#[test]
fn prune_dry_run_lists_orphans() {
    let tmp = TempDir::new().unwrap();
    setup_marketplace(&tmp, &["kept"], &["kept", "orphan1", "orphan2"]);
    let mp_path = tmp.path().join(".claude-plugin").join("marketplace.json");

    souk_cmd()
        .args(["prune", "--marketplace", mp_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("dry-run"))
        .stdout(predicate::str::contains("orphan1").or(predicate::str::contains("orphan2")))
        .stdout(predicate::str::contains("2 orphaned"));

    // Directories should still exist
    assert!(tmp.path().join("plugins").join("orphan1").exists());
    assert!(tmp.path().join("plugins").join("orphan2").exists());
}

#[test]
fn prune_apply_deletes_orphans() {
    let tmp = TempDir::new().unwrap();
    setup_marketplace(&tmp, &["kept"], &["kept", "orphan1", "orphan2"]);
    let mp_path = tmp.path().join(".claude-plugin").join("marketplace.json");

    souk_cmd()
        .args([
            "prune",
            "--apply",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deleted"))
        .stdout(predicate::str::contains("pruned"));

    // Orphans should be gone
    assert!(!tmp.path().join("plugins").join("orphan1").exists());
    assert!(!tmp.path().join("plugins").join("orphan2").exists());
    // Registered plugin should remain
    assert!(tmp.path().join("plugins").join("kept").exists());
}

#[test]
fn prune_nothing_to_do() {
    let tmp = TempDir::new().unwrap();
    setup_marketplace(&tmp, &["a"], &["a"]);
    let mp_path = tmp.path().join(".claude-plugin").join("marketplace.json");

    souk_cmd()
        .args(["prune", "--marketplace", mp_path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("No orphaned"));
}

#[test]
fn prune_json_output() {
    let tmp = TempDir::new().unwrap();
    setup_marketplace(&tmp, &["kept"], &["kept", "orphan"]);
    let mp_path = tmp.path().join(".claude-plugin").join("marketplace.json");

    souk_cmd()
        .args([
            "prune",
            "--json",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"results\""))
        .stdout(predicate::str::contains("orphan"));
}
