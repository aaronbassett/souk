//! Git hook integration for pre-commit and pre-push validation.
//!
//! This module provides functions that detect staged changes and run targeted
//! or full marketplace validation, designed to be called from git hooks
//! (`pre-commit` and `pre-push`).

use std::process::Command;

use crate::discovery::MarketplaceConfig;
use crate::error::{SoukError, ValidationDiagnostic, ValidationResult};
use crate::validation::{validate_marketplace, validate_plugin};

/// Detect which plugins have changes staged for commit.
///
/// Runs `git diff --cached --name-only` and matches paths against the
/// configured `pluginRoot`. Returns a deduplicated, sorted list of plugin
/// directory names that have at least one staged file.
///
/// # Errors
///
/// Returns `SoukError::Other` if the `git` command fails to execute or
/// exits with a non-zero status.
pub fn detect_changed_plugins(config: &MarketplaceConfig) -> Result<Vec<String>, SoukError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&config.project_root)
        .output()
        .map_err(|e| SoukError::Other(format!("Failed to run git: {e}")))?;

    if !output.status.success() {
        return Err(SoukError::Other("git diff failed".into()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let plugin_root_rel = config.marketplace.normalized_plugin_root();
    // Strip leading "./" from plugin root for matching against git paths
    let prefix = plugin_root_rel
        .strip_prefix("./")
        .unwrap_or(&plugin_root_rel);

    let mut plugin_names: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with(prefix) {
                // Extract the plugin directory name (first path component after prefix)
                let rest = line.strip_prefix(prefix)?.trim_start_matches('/');
                let name = rest.split('/').next()?;
                if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                }
            } else {
                None
            }
        })
        .collect();

    plugin_names.sort();
    plugin_names.dedup();

    Ok(plugin_names)
}

/// Check if marketplace.json is staged for commit.
///
/// Runs `git diff --cached --name-only` and looks for any staged file path
/// that ends with `marketplace.json`.
///
/// # Errors
///
/// Returns `SoukError::Other` if the `git` command fails to execute or
/// exits with a non-zero status.
pub fn is_marketplace_staged(config: &MarketplaceConfig) -> Result<bool, SoukError> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only"])
        .current_dir(&config.project_root)
        .output()
        .map_err(|e| SoukError::Other(format!("Failed to run git: {e}")))?;

    if !output.status.success() {
        return Err(SoukError::Other("git diff failed".into()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| line.contains("marketplace.json")))
}

/// Run pre-commit validation.
///
/// This validates only the plugins that have staged changes (detected via
/// `git diff --cached`). If `marketplace.json` itself is staged, the
/// marketplace structure is also validated (skipping individual plugin
/// validation to avoid redundancy).
///
/// Returns a [`ValidationResult`] that the caller can inspect to decide
/// whether to allow or block the commit.
pub fn run_pre_commit(config: &MarketplaceConfig) -> ValidationResult {
    let mut result = ValidationResult::new();

    // Get changed plugins
    let changed = match detect_changed_plugins(config) {
        Ok(names) => names,
        Err(e) => {
            result.push(ValidationDiagnostic::error(format!(
                "Failed to detect changed plugins: {e}"
            )));
            return result;
        }
    };

    // Validate each changed plugin
    for name in &changed {
        let plugin_path = config.plugin_root_abs.join(name);
        if plugin_path.is_dir() {
            let plugin_result = validate_plugin(&plugin_path);
            result.merge(plugin_result);
        }
    }

    // If marketplace.json is staged, validate marketplace structure
    if let Ok(true) = is_marketplace_staged(config) {
        let mp_result = validate_marketplace(config, true); // skip individual plugins
        result.merge(mp_result);
    }

    result
}

/// Run pre-push validation.
///
/// This performs a full marketplace validation including all plugins,
/// equivalent to `souk validate marketplace`. Use this in a `pre-push`
/// git hook to ensure only valid marketplaces are pushed to remote.
pub fn run_pre_push(config: &MarketplaceConfig) -> ValidationResult {
    validate_marketplace(config, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::load_marketplace_config;
    use tempfile::TempDir;

    /// Helper: create a valid marketplace inside a git repository.
    fn setup_git_marketplace(
        tmp: &TempDir,
        plugin_dirs: &[&str],
        plugins_json: &[(&str, &str)],
    ) -> MarketplaceConfig {
        // Initialize a git repo so `git diff` works
        Command::new("git")
            .args(["init"])
            .current_dir(tmp.path())
            .output()
            .expect("git init failed");

        let claude_dir = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        // Create plugin directories with valid manifests
        for name in plugin_dirs {
            let p = plugins_dir.join(name).join(".claude-plugin");
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(
                p.join("plugin.json"),
                format!(
                    r#"{{"name":"{name}","version":"1.0.0","description":"test plugin"}}"#
                ),
            )
            .unwrap();
        }

        // Build plugins array for marketplace.json
        let entries: Vec<String> = plugins_json
            .iter()
            .map(|(name, source)| format!(r#"{{"name":"{name}","source":"{source}"}}"#))
            .collect();
        let plugins_arr = entries.join(",");

        std::fs::write(
            claude_dir.join("marketplace.json"),
            format!(
                r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{plugins_arr}]}}"#
            ),
        )
        .unwrap();

        load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap()
    }

    #[test]
    fn pre_push_validates_entire_marketplace() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["my-plugin"], &[("my-plugin", "my-plugin")]);

        let result = run_pre_push(&config);
        assert!(
            !result.has_errors(),
            "Expected no errors, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn pre_push_catches_invalid_plugin() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["bad-plugin"], &[("bad-plugin", "bad-plugin")]);

        // Corrupt the plugin.json to trigger a validation error
        let plugin_json = tmp
            .path()
            .join("plugins/bad-plugin/.claude-plugin/plugin.json");
        std::fs::write(&plugin_json, "not valid json").unwrap();

        let result = run_pre_push(&config);
        assert!(result.has_errors());
    }

    #[test]
    fn pre_commit_returns_empty_when_no_staged_changes() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["my-plugin"], &[("my-plugin", "my-plugin")]);

        // No files staged, so pre-commit should produce no diagnostics
        let result = run_pre_commit(&config);
        assert!(
            !result.has_errors(),
            "Expected no errors for clean pre-commit, got: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.diagnostics.len(),
            0,
            "Expected zero diagnostics when nothing is staged"
        );
    }

    #[test]
    fn detect_changed_plugins_with_no_staged_files() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha"], &[("alpha", "alpha")]);

        let changed = detect_changed_plugins(&config).unwrap();
        assert!(changed.is_empty());
    }

    #[test]
    fn detect_changed_plugins_with_staged_plugin_file() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha", "beta"], &[("alpha", "alpha"), ("beta", "beta")]);

        // Stage a file inside alpha plugin
        let test_file = tmp.path().join("plugins/alpha/test.txt");
        std::fs::write(&test_file, "hello").unwrap();
        Command::new("git")
            .args(["add", "plugins/alpha/test.txt"])
            .current_dir(tmp.path())
            .output()
            .expect("git add failed");

        let changed = detect_changed_plugins(&config).unwrap();
        assert_eq!(changed, vec!["alpha"]);
    }

    #[test]
    fn detect_changed_plugins_deduplicates() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha"], &[("alpha", "alpha")]);

        // Stage two files inside the same plugin
        let file1 = tmp.path().join("plugins/alpha/a.txt");
        let file2 = tmp.path().join("plugins/alpha/b.txt");
        std::fs::write(&file1, "a").unwrap();
        std::fs::write(&file2, "b").unwrap();
        Command::new("git")
            .args(["add", "plugins/alpha/a.txt", "plugins/alpha/b.txt"])
            .current_dir(tmp.path())
            .output()
            .expect("git add failed");

        let changed = detect_changed_plugins(&config).unwrap();
        assert_eq!(changed, vec!["alpha"]);
    }

    #[test]
    fn is_marketplace_staged_returns_false_when_not_staged() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha"], &[("alpha", "alpha")]);

        assert!(!is_marketplace_staged(&config).unwrap());
    }

    #[test]
    fn is_marketplace_staged_returns_true_when_staged() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha"], &[("alpha", "alpha")]);

        // Stage the marketplace.json
        Command::new("git")
            .args(["add", ".claude-plugin/marketplace.json"])
            .current_dir(tmp.path())
            .output()
            .expect("git add failed");

        assert!(is_marketplace_staged(&config).unwrap());
    }

    #[test]
    fn pre_commit_validates_staged_plugin() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha"], &[("alpha", "alpha")]);

        // Stage a file in alpha
        let test_file = tmp.path().join("plugins/alpha/readme.txt");
        std::fs::write(&test_file, "some content").unwrap();
        Command::new("git")
            .args(["add", "plugins/alpha/readme.txt"])
            .current_dir(tmp.path())
            .output()
            .expect("git add failed");

        // alpha is valid, so pre-commit should pass
        let result = run_pre_commit(&config);
        assert!(
            !result.has_errors(),
            "Expected valid plugin to pass pre-commit: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn pre_commit_catches_invalid_staged_plugin() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["broken"], &[("broken", "broken")]);

        // Corrupt the plugin
        let plugin_json = tmp
            .path()
            .join("plugins/broken/.claude-plugin/plugin.json");
        std::fs::write(&plugin_json, "not json").unwrap();

        // Stage a file in broken
        let test_file = tmp.path().join("plugins/broken/file.txt");
        std::fs::write(&test_file, "content").unwrap();
        Command::new("git")
            .args(["add", "plugins/broken/file.txt"])
            .current_dir(tmp.path())
            .output()
            .expect("git add failed");

        let result = run_pre_commit(&config);
        assert!(result.has_errors());
    }

    #[test]
    fn pre_commit_validates_marketplace_when_staged() {
        let tmp = TempDir::new().unwrap();
        let config =
            setup_git_marketplace(&tmp, &["alpha"], &[("alpha", "alpha")]);

        // Stage marketplace.json
        Command::new("git")
            .args(["add", ".claude-plugin/marketplace.json"])
            .current_dir(tmp.path())
            .output()
            .expect("git add failed");

        // marketplace is valid, so should pass
        let result = run_pre_commit(&config);
        assert!(
            !result.has_errors(),
            "Expected valid marketplace to pass pre-commit: {:?}",
            result.diagnostics
        );
    }
}
