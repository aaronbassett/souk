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
            Err(e) => warnings.push(format!("Failed to delete {}: {e}", path.display())),
        }
    }

    Ok(PruneResult {
        orphaned,
        deleted,
        warnings,
    })
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

    #[cfg(unix)]
    #[test]
    fn prune_partial_failure_warns() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"kept","source":"kept"}]}"#,
            &["kept", "orphan1", "orphan2"],
        );

        // Delete one orphan directory before pruning so remove_dir_all will fail on it
        let orphan1_path = config.plugin_root_abs.join("orphan1");
        std::fs::remove_dir_all(&orphan1_path).unwrap();
        // Create a file at the same path so it's still found by read_dir but remove_dir_all
        // won't work as expected — actually, let's just remove it so it's gone from the scan.
        // Instead: make one orphan's directory unreadable.
        // On Linux, remove write permission from pluginRoot so deletion fails.
        use std::os::unix::fs::PermissionsExt;
        // Restore orphan1
        let p = orphan1_path.join(".claude-plugin");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("plugin.json"),
            r#"{"name":"orphan1","version":"1.0.0","description":"test"}"#,
        )
        .unwrap();

        // Make orphan1 non-deletable by removing write permission on it
        std::fs::set_permissions(&orphan1_path, std::fs::Permissions::from_mode(0o555)).unwrap();

        let result = prune_plugins(true, &config).unwrap();

        // Restore permissions for cleanup
        std::fs::set_permissions(&orphan1_path, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(result.orphaned.len(), 2);
        // One should succeed, one should fail
        assert_eq!(result.deleted.len(), 1);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("Failed to delete"));
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
