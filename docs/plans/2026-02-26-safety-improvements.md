# Safety Improvements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add safety guards across all mutation operations (add, remove, update) to prevent data loss, ensure transactional consistency, and improve error messaging.

**Architecture:** Validate-before-mutate pattern with extended AtomicGuard coverage. Restructure operation order so irreversible filesystem changes happen only after validation passes. Add path-boundary guards and symlink detection at the entry points.

**Tech Stack:** Rust, std::fs, std::time, std::process, tempfile (tests), serde_json

---

### Task 1: AtomicGuard Backup Collision Prevention

**Files:**
- Modify: `crates/souk-core/src/ops/atomic.rs:66-95` (AtomicGuard::new)
- Modify: `crates/souk-core/src/ops/atomic.rs:129-145` (Drop impl)

**Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `crates/souk-core/src/ops/atomic.rs`:

```rust
#[test]
fn rapid_guards_produce_unique_backups() {
    let dir = TempDir::new().expect("failed to create temp dir");
    let file_path = dir.path().join("marketplace.json");
    fs::write(&file_path, "original").unwrap();

    let guard1 = AtomicGuard::new(&file_path).unwrap();
    let guard2 = AtomicGuard::new(&file_path).unwrap();

    let bp1 = guard1.backup_path().unwrap().to_path_buf();
    let bp2 = guard2.backup_path().unwrap().to_path_buf();

    assert_ne!(bp1, bp2, "two guards created rapidly should have different backup paths");

    // Both backups should exist and contain the original content
    assert!(bp1.exists());
    assert!(bp2.exists());
    assert_eq!(fs::read_to_string(&bp1).unwrap(), "original");
    assert_eq!(fs::read_to_string(&bp2).unwrap(), "original");

    guard1.commit().unwrap();
    guard2.commit().unwrap();
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core rapid_guards_produce_unique_backups -- --nocapture`
Expected: FAIL — both guards produce the same backup path (same epoch second).

**Step 3: Write minimal implementation**

In `AtomicGuard::new`, change the epoch calculation from `as_secs()` to `as_nanos()` and append PID:

Replace lines 70-73 in `crates/souk-core/src/ops/atomic.rs`:
```rust
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before UNIX epoch")
                .as_nanos();
            let pid = std::process::id();
```

Replace the backup path construction (lines 75-82):
```rust
            let backup = original_path.with_extension(format!(
                "{}.bak.{}.{}",
                original_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or(""),
                nanos,
                pid
            ));
```

**Step 4: Add warning on restore failure in Drop**

Replace the Drop impl body (lines 130-144):
```rust
impl Drop for AtomicGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        if let Some(ref backup) = self.backup_path {
            if backup.exists() {
                if let Err(e) = fs::copy(backup, &self.original_path) {
                    eprintln!(
                        "Warning: failed to restore {} from backup {}: {}",
                        self.original_path.display(),
                        backup.display(),
                        e
                    );
                    return; // Don't remove backup if restore failed
                }
                if let Err(e) = fs::remove_file(backup) {
                    eprintln!(
                        "Warning: failed to remove backup file {}: {}",
                        backup.display(),
                        e
                    );
                }
            }
        }
    }
}
```

**Step 5: Run test to verify it passes**

Run: `cargo test -p souk-core rapid_guards_produce_unique_backups -- --nocapture`
Expected: PASS

**Step 6: Run full AtomicGuard test suite**

Run: `cargo test -p souk-core ops::atomic -- --nocapture`
Expected: ALL PASS. The `backup_path_includes_original_extension` test needs to be updated since the format changed.

Update the assertion in the `backup_path_includes_original_extension` test:
```rust
#[test]
fn backup_path_includes_original_extension() {
    let (_dir, file_path) = setup_file("test");

    let guard = AtomicGuard::new(&file_path).unwrap();
    let backup = guard.backup_path().unwrap();

    let backup_name = backup.file_name().unwrap().to_str().unwrap();
    assert!(
        backup_name.contains("json.bak."),
        "backup name '{backup_name}' should contain 'json.bak.'"
    );
    // Also verify PID is appended
    let pid = std::process::id().to_string();
    assert!(
        backup_name.ends_with(&pid),
        "backup name '{backup_name}' should end with PID '{pid}'"
    );

    guard.commit().unwrap();
}
```

**Step 7: Run full test suite again**

Run: `cargo test -p souk-core ops::atomic -- --nocapture`
Expected: ALL PASS

**Step 8: Commit**

```bash
git add crates/souk-core/src/ops/atomic.rs
git commit -m "fix: use nanos+PID for AtomicGuard backup names, warn on restore failure"
```

---

### Task 2: Symlink Detection in copy_dir_recursive

**Files:**
- Modify: `crates/souk-core/src/ops/add.rs:322-335` (copy_dir_recursive)

**Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests` in `crates/souk-core/src/ops/add.rs`:

```rust
#[cfg(unix)]
#[test]
fn copy_dir_recursive_rejects_symlinks() {
    let tmp = TempDir::new().unwrap();
    let src = tmp.path().join("src_plugin");
    let claude_dir = src.join(".claude-plugin");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("plugin.json"),
        r#"{"name":"sym","version":"1.0.0","description":"test"}"#,
    )
    .unwrap();

    // Create a symlink inside the plugin directory
    std::os::unix::fs::symlink("/tmp", src.join("bad-link")).unwrap();

    let dst = tmp.path().join("dst_plugin");
    let result = copy_dir_recursive(&src, &dst);

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Symlink"),
        "Error should mention symlink: {err_msg}"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core copy_dir_recursive_rejects_symlinks -- --nocapture`
Expected: FAIL — current implementation silently copies/skips the symlink.

**Step 3: Write minimal implementation**

Replace the `copy_dir_recursive` function in `crates/souk-core/src/ops/add.rs`:

```rust
/// Recursively copies a directory from `src` to `dst`.
///
/// Returns an error if any symlinks are encountered.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SoukError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        // Check for symlinks before processing
        let meta = fs::symlink_metadata(&src_path)?;
        if meta.file_type().is_symlink() {
            return Err(SoukError::Other(format!(
                "Symlink detected at '{}': symlinks are not supported in plugin directories",
                src_path.display()
            )));
        }

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p souk-core copy_dir_recursive_rejects_symlinks -- --nocapture`
Expected: PASS

**Step 5: Run all add tests to ensure no regression**

Run: `cargo test -p souk-core ops::add -- --nocapture`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add crates/souk-core/src/ops/add.rs
git commit -m "fix: reject symlinks in copy_dir_recursive during plugin add"
```

---

### Task 3: Delete Guard for `remove --delete`

**Files:**
- Modify: `crates/souk/src/cli.rs:59-67` (Remove variant)
- Modify: `crates/souk/src/commands/remove.rs` (run_remove)
- Modify: `crates/souk-core/src/ops/remove.rs:31-95` (remove_plugins)

**Step 3a: Add --allow-external-delete flag to CLI**

**Step 1: Modify CLI struct**

In `crates/souk/src/cli.rs`, add the flag to the Remove variant:

```rust
    /// Remove plugins from the marketplace
    Remove {
        /// Plugin names to remove
        plugins: Vec<String>,

        /// Also delete plugin directory from disk
        #[arg(long)]
        delete: bool,

        /// Allow deleting plugin directories outside pluginRoot
        #[arg(long, requires = "delete")]
        allow_external_delete: bool,
    },
```

**Step 2: Thread the flag through the command handler**

In `crates/souk/src/commands/remove.rs`, update the signature and call:

```rust
pub fn run_remove(
    plugins: &[String],
    delete: bool,
    allow_external_delete: bool,
    config: &MarketplaceConfig,
    reporter: &mut Reporter,
) -> bool {
    if plugins.is_empty() {
        reporter.error("At least one plugin name is required");
        return false;
    }

    reporter.section("Removing Plugins");

    match remove_plugins(plugins, delete, allow_external_delete, config) {
        Ok(removed) => {
            if removed.is_empty() {
                reporter.info("No plugins removed");
            } else {
                reporter.section("Summary");
                for name in &removed {
                    if delete {
                        reporter.success(&format!("Removed and deleted: {name}"));
                    } else {
                        reporter.success(&format!("Removed: {name}"));
                    }
                }
                reporter.success(&format!(
                    "Successfully removed {} plugin(s) from marketplace",
                    removed.len()
                ));
            }
            true
        }
        Err(e) => {
            reporter.error(&format!("Remove failed: {e}"));
            false
        }
    }
}
```

**Step 3: Update main.rs call site**

Find the match arm for `Commands::Remove` in `crates/souk/src/main.rs` and update to pass `allow_external_delete`. (The exact line depends on the match arm; grep for `Commands::Remove`.)

**Step 4: Commit CLI changes**

```bash
git add crates/souk/src/cli.rs crates/souk/src/commands/remove.rs crates/souk/src/main.rs
git commit -m "feat: add --allow-external-delete flag to remove command"
```

**Step 3b: Implement delete guard and reorder in core**

**Step 1: Write the failing tests**

Add to `#[cfg(test)] mod tests` in `crates/souk-core/src/ops/remove.rs`:

```rust
#[test]
fn remove_external_plugin_delete_refused_without_flag() {
    let tmp = TempDir::new().unwrap();

    // Create an external plugin directory
    let external_dir = TempDir::new().unwrap();
    let ext_plugin = external_dir.path().join("ext");
    let ext_claude = ext_plugin.join(".claude-plugin");
    fs::create_dir_all(&ext_claude).unwrap();
    fs::write(
        ext_claude.join("plugin.json"),
        r#"{"name":"ext","version":"1.0.0","description":"test"}"#,
    )
    .unwrap();

    // Set up marketplace with external source (absolute path)
    let claude_dir = tmp.path().join(".claude-plugin");
    fs::create_dir_all(&claude_dir).unwrap();
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let ext_path_str = ext_plugin.to_string_lossy();
    let mp_json = format!(
        r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{{"name":"ext","source":"{ext_path_str}"}}]}}"#
    );
    fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
    let config = load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap();

    // Try to delete without allow flag — should fail
    let result = remove_plugins(&["ext".to_string()], true, false, &config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("outside pluginRoot"), "Error: {err}");

    // External directory should still exist
    assert!(ext_plugin.exists());
}

#[test]
fn remove_external_plugin_delete_allowed_with_flag() {
    let tmp = TempDir::new().unwrap();

    let external_dir = TempDir::new().unwrap();
    let ext_plugin = external_dir.path().join("ext");
    let ext_claude = ext_plugin.join(".claude-plugin");
    fs::create_dir_all(&ext_claude).unwrap();
    fs::write(
        ext_claude.join("plugin.json"),
        r#"{"name":"ext","version":"1.0.0","description":"test"}"#,
    )
    .unwrap();

    let claude_dir = tmp.path().join(".claude-plugin");
    fs::create_dir_all(&claude_dir).unwrap();
    let plugins_dir = tmp.path().join("plugins");
    fs::create_dir_all(&plugins_dir).unwrap();

    let ext_path_str = ext_plugin.to_string_lossy();
    let mp_json = format!(
        r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{{"name":"ext","source":"{ext_path_str}"}}]}}"#
    );
    fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
    let config = load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap();

    // Delete with allow flag — should succeed
    let removed = remove_plugins(&["ext".to_string()], true, true, &config).unwrap();
    assert_eq!(removed, vec!["ext"]);
    assert!(!ext_plugin.exists());
}

#[test]
fn remove_internal_plugin_delete_works_without_flag() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

    assert!(config.plugin_root_abs.join("alpha").exists());

    let removed = remove_plugins(&["alpha".to_string()], true, false, &config).unwrap();
    assert_eq!(removed, vec!["alpha"]);
    assert!(!config.plugin_root_abs.join("alpha").exists());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p souk-core remove_external_plugin_delete -- --nocapture`
Expected: FAIL — compilation error, signature mismatch.

**Step 3: Implement the guarded remove_plugins**

Replace the entire `remove_plugins` function in `crates/souk-core/src/ops/remove.rs`:

```rust
pub fn remove_plugins(
    names: &[String],
    delete_files: bool,
    allow_external_delete: bool,
    config: &MarketplaceConfig,
) -> Result<Vec<String>, SoukError> {
    if names.is_empty() {
        return Ok(Vec::new());
    }

    // Verify all names exist before making any changes
    for name in names {
        if !config.marketplace.plugins.iter().any(|p| p.name == *name) {
            return Err(SoukError::PluginNotFound(name.clone()));
        }
    }

    // Pre-compute delete targets and validate paths before any mutation
    let mut delete_targets: Vec<(String, std::path::PathBuf)> = Vec::new();
    if delete_files {
        let plugin_root = config.plugin_root_abs.canonicalize().map_err(SoukError::Io)?;

        for name in names {
            let entry = config
                .marketplace
                .plugins
                .iter()
                .find(|p| p.name == *name)
                .unwrap();

            if let Ok(plugin_path) = resolve_source(&entry.source, config) {
                if plugin_path.is_dir() {
                    let resolved = plugin_path.canonicalize().map_err(SoukError::Io)?;
                    let is_internal = resolved.starts_with(&plugin_root);

                    if !is_internal && !allow_external_delete {
                        return Err(SoukError::Other(format!(
                            "Refusing to delete '{}': path is outside pluginRoot ({}). \
                             Use --allow-external-delete to override.",
                            resolved.display(),
                            plugin_root.display()
                        )));
                    }

                    delete_targets.push((name.clone(), resolved));
                }
            }
        }
    }

    // Atomic update — marketplace.json changes first
    let guard = AtomicGuard::new(&config.marketplace_path)?;

    let content = fs::read_to_string(&config.marketplace_path)?;
    let mut marketplace: Marketplace = serde_json::from_str(&content)?;

    let mut removed = Vec::new();
    for name in names {
        if marketplace.plugins.iter().any(|p| p.name == *name) {
            marketplace.plugins.retain(|p| p.name != *name);
            removed.push(name.clone());
        }
    }

    // Bump version
    marketplace.version = bump_patch(&marketplace.version)?;

    // Write back
    let json = serde_json::to_string_pretty(&marketplace)?;
    fs::write(&config.marketplace_path, format!("{json}\n"))?;

    // Validate
    let updated_config = load_marketplace_config(&config.marketplace_path)?;
    let validation = validate_marketplace(&updated_config, true);
    if validation.has_errors() {
        drop(guard);
        return Err(SoukError::AtomicRollback(
            "Validation failed after remove".to_string(),
        ));
    }

    guard.commit()?;

    // Delete directories AFTER successful marketplace update
    for (name, path) in &delete_targets {
        if path.is_dir() {
            if let Err(e) = fs::remove_dir_all(path) {
                eprintln!(
                    "Warning: removed '{name}' from marketplace but failed to delete directory {}: {e}",
                    path.display()
                );
            }
        }
    }

    Ok(removed)
}
```

Also update `delete_plugin_dir` to accept `allow_external_delete`:

```rust
pub fn delete_plugin_dir(
    source: &str,
    allow_external_delete: bool,
    config: &MarketplaceConfig,
) -> Result<(), SoukError> {
    let plugin_path = resolve_source(source, config)?;
    if plugin_path.is_dir() {
        let resolved = plugin_path.canonicalize().map_err(SoukError::Io)?;
        let plugin_root = config.plugin_root_abs.canonicalize().map_err(SoukError::Io)?;
        let is_internal = resolved.starts_with(&plugin_root);

        if !is_internal && !allow_external_delete {
            return Err(SoukError::Other(format!(
                "Refusing to delete '{}': path is outside pluginRoot ({}). \
                 Use --allow-external-delete to override.",
                resolved.display(),
                plugin_root.display()
            )));
        }

        fs::remove_dir_all(&resolved)?;
    }
    Ok(())
}
```

**Step 4: Fix existing tests**

Update existing test calls in `crates/souk-core/src/ops/remove.rs` to pass the new `allow_external_delete: false` parameter:

- `remove_existing_plugin`: change `remove_plugins(&["alpha".to_string()], false, &config)` to `remove_plugins(&["alpha".to_string()], false, false, &config)`
- `remove_nonexistent_plugin_returns_error`: same pattern
- `remove_with_delete_removes_directory`: change `true, &config` to `true, false, &config`
- `remove_without_delete_keeps_directory`: change `false, &config` to `false, false, &config`
- `remove_multiple_plugins`: same pattern
- `remove_empty_list_is_noop`: same pattern

**Step 5: Run all remove tests**

Run: `cargo test -p souk-core ops::remove -- --nocapture`
Expected: ALL PASS

**Step 6: Run full workspace build**

Run: `cargo build`
Expected: SUCCESS (after updating main.rs call site)

**Step 7: Commit**

```bash
git add crates/souk-core/src/ops/remove.rs
git commit -m "feat: guard deletes to prevent removing outside pluginRoot"
```

---

### Task 4: Rollback for `add` Copy Failures

**Files:**
- Modify: `crates/souk-core/src/ops/add.rs:183-291` (execute_add)

**Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests` in `crates/souk-core/src/ops/add.rs`:

```rust
#[test]
fn add_cleans_up_copied_dir_on_marketplace_failure() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace(&tmp, "");

    // Create external plugin
    let external_dir = TempDir::new().unwrap();
    create_plugin(external_dir.path(), "ext-plugin");
    let ext_path = external_dir.path().join("ext-plugin");

    let plan = plan_add(
        &[ext_path.to_string_lossy().to_string()],
        &config,
        "abort",
        false,
    )
    .unwrap();

    // Corrupt marketplace.json so validation will fail after copy
    fs::write(&config.marketplace_path, "not valid json").unwrap();

    let result = execute_add(&plan, &config, false);
    assert!(result.is_err());

    // The copied directory should have been cleaned up
    let would_be_copied = config.plugin_root_abs.join("ext-plugin");
    assert!(
        !would_be_copied.exists(),
        "Copied dir should be cleaned up on failure"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core add_cleans_up_copied_dir_on_marketplace_failure -- --nocapture`
Expected: FAIL — copied directory remains after failure.

**Step 3: Write minimal implementation**

In `execute_add` in `crates/souk-core/src/ops/add.rs`, add a `copied_dirs` tracker and cleanup on error. Replace the copy phase and add cleanup in error paths:

After the dry-run gate (around line 210), add:
```rust
    // Track directories we copy so we can clean up on failure
    let mut copied_dirs: Vec<PathBuf> = Vec::new();
```

In the copy loop (around line 212-234), after each `copy_dir_recursive` call:
```rust
            copy_dir_recursive(&action.plugin_path, &target_dir)?;
            copied_dirs.push(target_dir);
```

Then wrap the rest of execute_add (atomic update through validation) in a closure or use a helper to ensure cleanup. The simplest approach is to use a nested function or match:

Replace the section from Phase 5 through the end of the function with:

```rust
    // Phase 5-7: Atomic update, version bump, validation
    let result = execute_add_marketplace(&effective_actions, config, &copied_dirs);

    if result.is_err() {
        // Clean up copied directories on failure
        for dir in &copied_dirs {
            let _ = fs::remove_dir_all(dir);
        }
    }

    result
```

Add a helper function right before `execute_add`:

```rust
/// Inner marketplace mutation, separated for cleanup-on-failure in execute_add.
fn execute_add_marketplace(
    effective_actions: &[&AddAction],
    config: &MarketplaceConfig,
    _copied_dirs: &[PathBuf],
) -> Result<Vec<String>, SoukError> {
    let guard = AtomicGuard::new(&config.marketplace_path)?;

    let content = fs::read_to_string(&config.marketplace_path)?;
    let mut marketplace: Marketplace = serde_json::from_str(&content)?;

    let mut added_names = Vec::new();

    for action in effective_actions {
        let (final_name, final_source) = match &action.conflict {
            Some(ConflictResolution::Replace) => {
                marketplace.plugins.retain(|p| p.name != action.plugin_name);
                (action.plugin_name.clone(), action.source.clone())
            }
            Some(ConflictResolution::Rename(new_name)) => (new_name.clone(), new_name.clone()),
            Some(ConflictResolution::Skip) => continue,
            None => (action.plugin_name.clone(), action.source.clone()),
        };

        let manifest = read_plugin_manifest(&action.plugin_path)?;
        let tags = manifest.keywords;

        marketplace.plugins.push(PluginEntry {
            name: final_name.clone(),
            source: final_source,
            tags,
        });

        added_names.push(final_name);
    }

    marketplace.version = bump_patch(&marketplace.version)?;

    let json = serde_json::to_string_pretty(&marketplace)?;
    fs::write(&config.marketplace_path, format!("{json}\n"))?;

    let updated_config = load_marketplace_config(&config.marketplace_path)?;
    let validation = validate_marketplace(&updated_config, true);
    if validation.has_errors() {
        drop(guard);
        return Err(SoukError::AtomicRollback(
            "Final validation failed after add".to_string(),
        ));
    }

    guard.commit()?;

    Ok(added_names)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p souk-core add_cleans_up_copied_dir_on_marketplace_failure -- --nocapture`
Expected: PASS

**Step 5: Run all add tests**

Run: `cargo test -p souk-core ops::add -- --nocapture`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add crates/souk-core/src/ops/add.rs
git commit -m "fix: clean up copied dirs on marketplace validation failure in add"
```

---

### Task 5: Transactional Update for plugin.json Edits

**Files:**
- Modify: `crates/souk-core/src/ops/update.rs:35-156` (update_plugins)

**Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests` in `crates/souk-core/src/ops/update.rs`:

```rust
#[test]
fn update_bump_rolls_back_plugin_json_on_validation_failure() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

    // Record original plugin.json content
    let plugin_json_path = config
        .plugin_root_abs
        .join("alpha")
        .join(".claude-plugin")
        .join("plugin.json");
    let original_content = fs::read_to_string(&plugin_json_path).unwrap();

    // Corrupt marketplace.json so post-update validation will fail
    // (write invalid version after the guard is created but before validation)
    // Simpler: make plugin validation fail by breaking the plugin AFTER bump
    // Actually, let's corrupt marketplace.json so reading it back fails
    // We need the guard to be created first, so let's break it differently.
    //
    // Strategy: remove the plugin directory after initial resolve but before
    // validation. This is hard to do from outside. Instead, let's use a
    // different approach: add a second plugin with a duplicate name in
    // marketplace.json that will trigger validation error.

    // Create a marketplace with duplicate names (alpha appears twice)
    let claude_dir = tmp.path().join(".claude-plugin");
    let mp_json = r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[
        {"name":"alpha","source":"alpha","tags":["old"]},
        {"name":"alpha","source":"alpha","tags":["dup"]}
    ]}"#;
    fs::write(claude_dir.join("marketplace.json"), mp_json).unwrap();
    let bad_config = load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap();

    // This should fail because the marketplace has duplicate names
    let result = update_plugins(&["alpha".to_string()], Some("patch"), &bad_config);
    assert!(result.is_err());

    // plugin.json should be restored to original version
    let restored = fs::read_to_string(&plugin_json_path).unwrap();
    let manifest: PluginManifest = serde_json::from_str(&restored).unwrap();
    assert_eq!(
        manifest.version_str(),
        Some("1.0.0"),
        "plugin.json should be rolled back to original version"
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core update_bump_rolls_back_plugin_json -- --nocapture`
Expected: FAIL — plugin.json retains the bumped "1.0.1" version after rollback.

**Step 3: Implement transactional update**

Replace the `update_plugins` function in `crates/souk-core/src/ops/update.rs`:

```rust
pub fn update_plugins(
    names: &[String],
    bump_type: Option<&str>,
    config: &MarketplaceConfig,
) -> Result<Vec<String>, SoukError> {
    if names.is_empty() {
        return Ok(Vec::new());
    }

    // Verify all names exist
    for name in names {
        if !config.marketplace.plugins.iter().any(|p| p.name == *name) {
            return Err(SoukError::PluginNotFound(name.clone()));
        }
    }

    // Resolve all plugin paths first (fail fast)
    let mut plugin_paths: Vec<(String, std::path::PathBuf)> = Vec::new();
    for name in names {
        let entry = config
            .marketplace
            .plugins
            .iter()
            .find(|p| p.name == *name)
            .unwrap();
        let plugin_path = resolve_source(&entry.source, config)?;
        plugin_paths.push((name.clone(), plugin_path));
    }

    // Create ALL guards BEFORE any writes
    let mp_guard = AtomicGuard::new(&config.marketplace_path)?;

    let mut plugin_guards: Vec<AtomicGuard> = Vec::new();
    if bump_type.is_some() {
        for (_name, plugin_path) in &plugin_paths {
            let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");
            let guard = AtomicGuard::new(&plugin_json_path)?;
            plugin_guards.push(guard);
        }
    }

    // Now perform version bumps (protected by guards)
    if let Some(bump) = bump_type {
        for (name, plugin_path) in &plugin_paths {
            let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");
            let content = fs::read_to_string(&plugin_json_path).map_err(|e| {
                SoukError::Other(format!("Cannot read plugin.json for {name}: {e}"))
            })?;

            let mut doc: serde_json::Value = serde_json::from_str(&content)?;

            if let Some(version) = doc.get("version").and_then(|v| v.as_str()) {
                let new_version = match bump {
                    "major" => bump_major(version)?,
                    "minor" => bump_minor(version)?,
                    "patch" => bump_patch(version)?,
                    _ => {
                        return Err(SoukError::Other(format!("Invalid bump type: {bump}")));
                    }
                };
                doc["version"] = serde_json::Value::String(new_version);
            }

            let updated_json = serde_json::to_string_pretty(&doc)?;
            fs::write(&plugin_json_path, format!("{updated_json}\n"))?;
        }
    }

    // Update marketplace entries
    let content = fs::read_to_string(&config.marketplace_path)?;
    let mut marketplace: Marketplace = serde_json::from_str(&content)?;

    let mut updated = Vec::new();

    for (name, plugin_path) in &plugin_paths {
        let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");
        let pj_content = fs::read_to_string(&plugin_json_path)
            .map_err(|e| SoukError::Other(format!("Cannot read plugin.json for {name}: {e}")))?;

        let manifest: PluginManifest = serde_json::from_str(&pj_content)?;

        // Check for rename collisions
        if let Some(new_name) = manifest.name_str() {
            if new_name != name.as_str() {
                let collides = marketplace
                    .plugins
                    .iter()
                    .any(|p| p.name == new_name && !names.contains(&p.name));
                if collides {
                    // Drop all guards to trigger rollback
                    drop(mp_guard);
                    for g in plugin_guards {
                        drop(g);
                    }
                    return Err(SoukError::Other(format!(
                        "Plugin '{name}' would be renamed to '{new_name}' which conflicts with an existing plugin"
                    )));
                }
            }
        }

        if let Some(entry) = marketplace.plugins.iter_mut().find(|p| p.name == *name) {
            entry.tags = manifest.keywords.clone();
            if let Some(new_name) = manifest.name_str() {
                if new_name != name.as_str() {
                    entry.name = new_name.to_string();
                }
            }
        }

        let validation = validate_plugin(plugin_path);
        if validation.has_errors() {
            // Guards drop automatically, restoring all files
            drop(mp_guard);
            for g in plugin_guards {
                drop(g);
            }
            return Err(SoukError::AtomicRollback(format!(
                "Plugin validation failed for {name} after update"
            )));
        }

        updated.push(name.clone());
    }

    // Bump marketplace version
    marketplace.version = bump_patch(&marketplace.version)?;

    // Write back
    let json = serde_json::to_string_pretty(&marketplace)?;
    fs::write(&config.marketplace_path, format!("{json}\n"))?;

    // Final validation
    let updated_config = load_marketplace_config(&config.marketplace_path)?;
    let validation = validate_marketplace(&updated_config, true);
    if validation.has_errors() {
        drop(mp_guard);
        for g in plugin_guards {
            drop(g);
        }
        return Err(SoukError::AtomicRollback(
            "Marketplace validation failed after update".to_string(),
        ));
    }

    // Success — commit all guards
    mp_guard.commit()?;
    for g in plugin_guards {
        g.commit()?;
    }

    Ok(updated)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p souk-core update_bump_rolls_back_plugin_json -- --nocapture`
Expected: PASS

**Step 5: Run all update tests**

Run: `cargo test -p souk-core ops::update -- --nocapture`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add crates/souk-core/src/ops/update.rs
git commit -m "fix: make update transactional, protect plugin.json with AtomicGuard"
```

---

### Task 6: Rename Collision Detection in Update

This is already implemented as part of Task 5 above (the `manifest.name_str()` collision check). We just need a dedicated test.

**Step 1: Write the test**

Add to `#[cfg(test)] mod tests` in `crates/souk-core/src/ops/update.rs`:

```rust
#[test]
fn update_detects_rename_collision() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta"]);

    // Modify alpha's plugin.json to have name "beta" (which already exists)
    let alpha_pj = config
        .plugin_root_abs
        .join("alpha")
        .join(".claude-plugin")
        .join("plugin.json");
    fs::write(
        &alpha_pj,
        r#"{"name":"beta","version":"1.0.0","description":"test plugin","keywords":["original"]}"#,
    )
    .unwrap();

    // Update alpha — should detect the rename collision with beta
    let result = update_plugins(&["alpha".to_string()], None, &config);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("conflicts"),
        "Should report rename collision: {err}"
    );

    // marketplace.json should be unchanged (rolled back)
    let content = fs::read_to_string(&config.marketplace_path).unwrap();
    let mp: Marketplace = serde_json::from_str(&content).unwrap();
    assert_eq!(mp.plugins.len(), 2);
    assert!(mp.plugins.iter().any(|p| p.name == "alpha"));
    assert!(mp.plugins.iter().any(|p| p.name == "beta"));
}
```

**Step 2: Run test**

Run: `cargo test -p souk-core update_detects_rename_collision -- --nocapture`
Expected: PASS (already implemented in Task 5)

**Step 3: Commit**

```bash
git add crates/souk-core/src/ops/update.rs
git commit -m "test: add rename collision detection test for update"
```

---

### Task 7: Consistent Error Handling for Source-of-Truth Drift

**Files:**
- Modify: `crates/souk-core/src/validation/marketplace.rs:96-143` (check_completeness)

**Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests` in `crates/souk-core/src/validation/marketplace.rs`:

```rust
#[test]
fn marketplace_not_in_filesystem_includes_remediation_hint() {
    let tmp = TempDir::new().unwrap();
    let config = setup_marketplace(
        &tmp,
        r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"ghost","source":"ghost"}]}"#,
        &[],
    );
    let result = validate_marketplace(&config, true);
    assert!(result.has_errors());
    let err = result
        .diagnostics
        .iter()
        .find(|d| d.is_error() && d.message.contains("ghost"))
        .expect("Should have error for missing plugin");
    assert!(
        err.message.contains("souk remove"),
        "Error should include remediation hint: {}",
        err.message
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p souk-core marketplace_not_in_filesystem_includes_remediation_hint -- --nocapture`
Expected: FAIL — current error message doesn't include remediation hint.

**Step 3: Update check_completeness**

In `crates/souk-core/src/validation/marketplace.rs`, update the marketplace-not-in-filesystem error message in `check_completeness`:

```rust
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
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p souk-core marketplace_not_in_filesystem_includes_remediation_hint -- --nocapture`
Expected: PASS

**Step 5: Run all validation tests**

Run: `cargo test -p souk-core validation::marketplace -- --nocapture`
Expected: ALL PASS. Check that `completeness_marketplace_not_in_filesystem` still passes (its assertion checks `contains("not in filesystem")` which is still present in the new message).

**Step 6: Commit**

```bash
git add crates/souk-core/src/validation/marketplace.rs
git commit -m "fix: add remediation hints to source-of-truth drift error messages"
```

---

### Task 8: Final Integration Verification

**Step 1: Run the full test suite**

Run: `cargo test --workspace`
Expected: ALL PASS

**Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings

**Step 3: Check formatting**

Run: `cargo fmt --check`
Expected: No formatting issues (run `cargo fmt` if needed)

**Step 4: Commit any fixes**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting"
```
