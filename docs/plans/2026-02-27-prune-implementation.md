# souk prune — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `souk prune` command that removes orphaned plugin directories not listed in marketplace.json, with dry-run by default and `--apply` to delete.

**Architecture:** Extract a shared `find_orphaned_dirs()` helper from the validation module. Build a new `prune` ops module that calls the helper and optionally deletes. Wire it into the CLI with a new `Prune` command variant. Pure filesystem operation — no marketplace.json mutation.

**Tech Stack:** Rust, clap v4 (derive), std::fs, tempfile (tests), assert_cmd + predicates (integration tests)

---

### Task 1: Extract `find_orphaned_dirs()` shared helper

**Files:**
- Modify: `crates/souk-core/src/validation/marketplace.rs:89-144`

**Step 1: Write the failing test**

Add a test for the new public function in `crates/souk-core/src/validation/marketplace.rs` inside the existing `mod tests` block (after line 302):

```rust
#[test]
fn find_orphaned_dirs_returns_correct_paths() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace(
        &tmp,
        r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"kept","source":"kept"}]}"#,
        &["kept", "orphan1", "orphan2"],
    );
    let orphans = find_orphaned_dirs(&config).unwrap();
    assert_eq!(orphans.len(), 2);
    let names: Vec<String> = orphans
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert!(names.contains(&"orphan1".to_string()));
    assert!(names.contains(&"orphan2".to_string()));
}

#[test]
fn find_orphaned_dirs_empty_when_all_registered() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace(
        &tmp,
        r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"a","source":"a"}]}"#,
        &["a"],
    );
    let orphans = find_orphaned_dirs(&config).unwrap();
    assert!(orphans.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core find_orphaned_dirs -- --nocapture`
Expected: FAIL — `find_orphaned_dirs` is not defined

**Step 3: Extract the helper function**

In `crates/souk-core/src/validation/marketplace.rs`, add a new **public** function before `check_completeness` (before line 89). This extracts the orphan-detection logic:

```rust
/// Returns full paths of directories under pluginRoot that are not listed in marketplace.json.
///
/// Scans the plugin root directory and compares against the marketplace entries.
/// Used by both validation (to warn) and prune (to delete).
pub fn find_orphaned_dirs(config: &MarketplaceConfig) -> Result<Vec<std::path::PathBuf>, crate::error::SoukError> {
    let fs_plugins: HashSet<String> = match std::fs::read_dir(&config.plugin_root_abs) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(e) => return Err(crate::error::SoukError::Io(e)),
    };

    let mp_sources: HashSet<String> = config
        .marketplace
        .plugins
        .iter()
        .map(|p| {
            Path::new(&p.source)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.source.clone())
        })
        .collect();

    let orphans = fs_plugins
        .iter()
        .filter(|name| !mp_sources.contains(*name))
        .map(|name| config.plugin_root_abs.join(name))
        .collect();

    Ok(orphans)
}
```

Then refactor `check_completeness` to call `find_orphaned_dirs`:

```rust
fn check_completeness(config: &MarketplaceConfig) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Orphaned dirs on filesystem
    match find_orphaned_dirs(config) {
        Ok(orphans) => {
            for path in orphans {
                let name = path.file_name().unwrap().to_string_lossy();
                result.push(
                    ValidationDiagnostic::warning(format!(
                        "Plugin in filesystem but not in marketplace: {name}"
                    ))
                    .with_path(&path),
                );
            }
        }
        Err(_) => return result,
    }

    // Missing dirs from marketplace (keep existing logic)
    let fs_plugins: HashSet<String> = match std::fs::read_dir(&config.plugin_root_abs) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect(),
        Err(_) => return result,
    };

    let mp_sources: HashSet<String> = config
        .marketplace
        .plugins
        .iter()
        .map(|p| {
            Path::new(&p.source)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.source.clone())
        })
        .collect();

    for mp_source in &mp_sources {
        if !fs_plugins.contains(mp_source) {
            result.push(
                ValidationDiagnostic::error(format!(
                    "Plugin in marketplace but not in filesystem: {mp_source}. \
                     Run `souk remove {mp_source}` to clean up the stale entry."
                ))
                .with_path(&config.marketplace_path),
            );
        }
    }

    result
}
```

Also add the re-export in `crates/souk-core/src/validation/mod.rs`:

```rust
pub use marketplace::find_orphaned_dirs;
```

**Step 4: Run tests to verify everything passes**

Run: `cargo test -p souk-core -- --nocapture`
Expected: ALL PASS — both new tests and all existing validation tests

**Step 5: Commit**

```bash
git add crates/souk-core/src/validation/marketplace.rs crates/souk-core/src/validation/mod.rs
git commit -m "refactor: extract find_orphaned_dirs helper from validation"
```

---

### Task 2: Create prune core operation with unit tests

**Files:**
- Create: `crates/souk-core/src/ops/prune.rs`
- Modify: `crates/souk-core/src/ops/mod.rs:8` — add `pub mod prune;`

**Step 1: Write the failing tests**

Create `crates/souk-core/src/ops/prune.rs` with tests only:

```rust
//! Prune orphaned plugin directories from the filesystem.
//!
//! Identifies directories under pluginRoot that are not listed in
//! marketplace.json and optionally deletes them.

use std::fs;
use std::path::PathBuf;

use crate::discovery::MarketplaceConfig;
use crate::error::SoukError;
use crate::validation::find_orphaned_dirs;

/// The result of a prune operation.
#[derive(Debug)]
pub struct PruneResult {
    /// Orphaned directories found.
    pub orphaned: Vec<PathBuf>,
    /// Directories actually deleted (empty if dry-run).
    pub deleted: Vec<PathBuf>,
    /// Non-fatal warnings (e.g., permission denied on delete).
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::load_marketplace_config;
    use tempfile::TempDir;

    fn setup_marketplace(tmp: &TempDir, json: &str, plugin_dirs: &[&str]) -> MarketplaceConfig {
        let claude = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        let plugins = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();

        for name in plugin_dirs {
            let p = plugins.join(name).join(".claude-plugin");
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(
                p.join("plugin.json"),
                format!(r#"{{"name":"{name}","version":"1.0.0","description":"test"}}"#),
            )
            .unwrap();
        }

        std::fs::write(claude.join("marketplace.json"), json).unwrap();
        load_marketplace_config(&claude.join("marketplace.json")).unwrap()
    }

    #[test]
    fn prune_dry_run_lists_orphans() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"kept","source":"kept"}]}"#,
            &["kept", "orphan1", "orphan2"],
        );

        let result = prune_plugins(false, &config).unwrap();

        assert_eq!(result.orphaned.len(), 2);
        assert!(result.deleted.is_empty());
        // Directories should still exist
        assert!(config.plugin_root_abs.join("orphan1").exists());
        assert!(config.plugin_root_abs.join("orphan2").exists());
    }

    #[test]
    fn prune_apply_deletes_orphans() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"kept","source":"kept"}]}"#,
            &["kept", "orphan1", "orphan2"],
        );

        let result = prune_plugins(true, &config).unwrap();

        assert_eq!(result.orphaned.len(), 2);
        assert_eq!(result.deleted.len(), 2);
        assert!(result.warnings.is_empty());
        // Orphans should be gone
        assert!(!config.plugin_root_abs.join("orphan1").exists());
        assert!(!config.plugin_root_abs.join("orphan2").exists());
        // Registered plugin should still exist
        assert!(config.plugin_root_abs.join("kept").exists());
    }

    #[test]
    fn prune_no_orphans() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"a","source":"a"}]}"#,
            &["a"],
        );

        let result = prune_plugins(false, &config).unwrap();

        assert!(result.orphaned.is_empty());
        assert!(result.deleted.is_empty());
        assert!(result.warnings.is_empty());
    }
}
```

Register the module in `crates/souk-core/src/ops/mod.rs` — add `pub mod prune;` after line 11.

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core prune -- --nocapture`
Expected: FAIL — `prune_plugins` is not defined

**Step 3: Write minimal implementation**

Add the `prune_plugins` function in `crates/souk-core/src/ops/prune.rs` (after `PruneResult`, before `#[cfg(test)]`):

```rust
/// Prunes orphaned plugin directories.
///
/// Finds directories under pluginRoot not listed in marketplace.json.
/// If `apply` is false (dry-run), only reports what would be deleted.
/// If `apply` is true, actually deletes the orphaned directories.
///
/// This is a pure filesystem operation — marketplace.json is not modified.
pub fn prune_plugins(apply: bool, config: &MarketplaceConfig) -> Result<PruneResult, SoukError> {
    let orphaned = find_orphaned_dirs(config)?;

    if !apply {
        return Ok(PruneResult {
            orphaned,
            deleted: Vec::new(),
            warnings: Vec::new(),
        });
    }

    let mut deleted = Vec::new();
    let mut warnings = Vec::new();

    for path in &orphaned {
        match fs::remove_dir_all(path) {
            Ok(()) => deleted.push(path.clone()),
            Err(e) => warnings.push(format!(
                "Failed to delete {}: {e}",
                path.display()
            )),
        }
    }

    Ok(PruneResult {
        orphaned,
        deleted,
        warnings,
    })
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p souk-core prune -- --nocapture`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add crates/souk-core/src/ops/prune.rs crates/souk-core/src/ops/mod.rs
git commit -m "feat: add prune_plugins core operation"
```

---

### Task 3: Wire up CLI — add `Prune` command variant and handler

**Files:**
- Create: `crates/souk/src/commands/prune.rs`
- Modify: `crates/souk/src/cli.rs:119` — add `Prune` variant before closing brace of `Commands`
- Modify: `crates/souk/src/commands/mod.rs` — add `pub mod prune;`
- Modify: `crates/souk/src/main.rs:137` — add match arm before closing brace

**Step 1: Write the CLI handler**

Create `crates/souk/src/commands/prune.rs`:

```rust
//! Handler for the `souk prune` CLI command.

use crate::output::Reporter;
use souk_core::discovery::MarketplaceConfig;
use souk_core::ops::prune::prune_plugins;

/// Run the prune command, removing orphaned plugin directories.
///
/// Returns `true` on success, `false` on failure.
pub fn run_prune(apply: bool, config: &MarketplaceConfig, reporter: &mut Reporter) -> bool {
    match prune_plugins(apply, config) {
        Ok(result) => {
            if result.orphaned.is_empty() {
                reporter.info("No orphaned plugin directories found.");
                return true;
            }

            if apply {
                reporter.section("Prune");
                for path in &result.deleted {
                    let name = path.file_name().unwrap().to_string_lossy();
                    reporter.success(&format!("Deleted: {name}"));
                }
                for warn in &result.warnings {
                    reporter.warning(warn);
                }
                reporter.success(&format!(
                    "Successfully pruned {} orphaned plugin directory(ies).",
                    result.deleted.len()
                ));
            } else {
                reporter.section("Prune (dry-run)");
                for path in &result.orphaned {
                    let name = path.file_name().unwrap().to_string_lossy();
                    reporter.info(&format!("Would delete: {name}"));
                }
                reporter.info(&format!(
                    "Found {} orphaned plugin directory(ies). Run with --apply to delete.",
                    result.orphaned.len()
                ));
            }

            true
        }
        Err(e) => {
            reporter.error(&format!("Prune failed: {e}"));
            false
        }
    }
}
```

**Step 2: Add CLI variant**

In `crates/souk/src/cli.rs`, add inside the `Commands` enum (before the closing brace at line 119):

```rust
    /// Remove orphaned plugin directories not listed in marketplace.json
    Prune {
        /// Actually delete orphaned directories (default: dry-run)
        #[arg(long)]
        apply: bool,
    },
```

**Step 3: Register the module**

In `crates/souk/src/commands/mod.rs`, add:

```rust
pub mod prune;
```

**Step 4: Add the match arm in main.rs**

In `crates/souk/src/main.rs`, add a new match arm (before the `Commands::Completions` arm at line 127):

```rust
        Commands::Prune { apply } => match load_config_required(marketplace, &mut reporter) {
            Some(config) => commands::prune::run_prune(apply, &config, &mut reporter),
            None => false,
        },
```

**Step 5: Verify it compiles**

Run: `cargo build`
Expected: SUCCESS

**Step 6: Commit**

```bash
git add crates/souk/src/commands/prune.rs crates/souk/src/commands/mod.rs crates/souk/src/cli.rs crates/souk/src/main.rs
git commit -m "feat: wire up souk prune CLI command"
```

---

### Task 4: Add integration tests

**Files:**
- Create: `crates/souk/tests/prune_test.rs`

**Step 1: Write integration tests**

Create `crates/souk/tests/prune_test.rs`:

```rust
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
        .args([
            "prune",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
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
        .args([
            "prune",
            "--marketplace",
            mp_path.to_str().unwrap(),
        ])
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
```

**Step 2: Run integration tests**

Run: `cargo test -p souk --test prune_test -- --nocapture`
Expected: ALL PASS

**Step 3: Run full test suite**

Run: `cargo test`
Expected: ALL PASS — no regressions in existing tests

**Step 4: Commit**

```bash
git add crates/souk/tests/prune_test.rs
git commit -m "test: add integration tests for souk prune command"
```

---

### Task 5: Final verification and cleanup

**Step 1: Run the full test suite one more time**

Run: `cargo test`
Expected: ALL PASS

**Step 2: Run clippy for lint checks**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Run rustfmt**

Run: `cargo fmt --check`
Expected: No formatting issues (if any, run `cargo fmt` to fix)

**Step 4: Manual smoke test**

Run: `cargo run -- prune --help`
Expected: Shows help text with `--apply` flag documented

**Step 5: Commit any cleanup if needed**

If clippy or fmt required changes:

```bash
git add -A
git commit -m "style: fix formatting and lint warnings"
```
