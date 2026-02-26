//! Remove plugins from the marketplace.
//!
//! Removes one or more plugins by name from `marketplace.json`, with an
//! optional flag to also delete the plugin directory from disk.

use std::fs;

use crate::discovery::{load_marketplace_config, MarketplaceConfig};
use crate::error::SoukError;
use crate::ops::AtomicGuard;
use crate::resolution::resolve_source;
use crate::types::Marketplace;
use crate::validation::validate_marketplace;
use crate::version::bump_patch;

/// Removes the named plugins from the marketplace.
///
/// For each name in `names`:
/// - Finds the matching entry in marketplace.json
/// - If `delete_files` is true, also removes the plugin directory from disk
/// - Bumps the marketplace version (patch)
///
/// Returns the list of plugin names that were actually removed.
///
/// # Errors
///
/// Returns [`SoukError::PluginNotFound`] if any name does not exist in
/// the marketplace.
///
/// Returns [`SoukError::AtomicRollback`] if the post-removal validation fails.
pub fn remove_plugins(
    names: &[String],
    delete_files: bool,
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

    // Atomic update
    let guard = AtomicGuard::new(&config.marketplace_path)?;

    let content = fs::read_to_string(&config.marketplace_path)?;
    let mut marketplace: Marketplace = serde_json::from_str(&content)?;

    let mut removed = Vec::new();

    for name in names {
        // Find the entry to get the source path before removing
        let entry = marketplace.plugins.iter().find(|p| p.name == *name);

        if let Some(entry) = entry {
            if delete_files {
                // Resolve the source to a filesystem path
                if let Ok(plugin_path) = resolve_source(&entry.source, config) {
                    if plugin_path.is_dir() {
                        fs::remove_dir_all(&plugin_path)?;
                    }
                }
            }

            // Remove from plugins list
            marketplace.plugins.retain(|p| p.name != *name);
            removed.push(name.clone());
        }
    }

    // Bump version
    marketplace.version = bump_patch(&marketplace.version)?;

    // Write back
    let json = serde_json::to_string_pretty(&marketplace)?;
    fs::write(&config.marketplace_path, format!("{json}\n"))?;

    // Validate (skip plugin-level validation since we just removed plugins)
    let updated_config = load_marketplace_config(&config.marketplace_path)?;
    let validation = validate_marketplace(&updated_config, true);
    if validation.has_errors() {
        drop(guard);
        return Err(SoukError::AtomicRollback(
            "Validation failed after remove".to_string(),
        ));
    }

    guard.commit()?;

    Ok(removed)
}

/// Deletes a plugin directory from disk. Exposed for testing or direct use.
pub fn delete_plugin_dir(source: &str, config: &MarketplaceConfig) -> Result<(), SoukError> {
    let plugin_path = resolve_source(source, config)?;
    if plugin_path.is_dir() {
        fs::remove_dir_all(&plugin_path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::load_marketplace_config;
    use tempfile::TempDir;

    fn setup_marketplace_with_plugins(
        tmp: &TempDir,
        plugin_names: &[&str],
    ) -> MarketplaceConfig {
        let claude_dir = tmp.path().join(".claude-plugin");
        fs::create_dir_all(&claude_dir).unwrap();
        let plugins_dir = tmp.path().join("plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        let mut entries = Vec::new();
        for name in plugin_names {
            // Create plugin directory
            let plugin_dir = plugins_dir.join(name);
            let plugin_claude = plugin_dir.join(".claude-plugin");
            fs::create_dir_all(&plugin_claude).unwrap();
            fs::write(
                plugin_claude.join("plugin.json"),
                format!(
                    r#"{{"name":"{name}","version":"1.0.0","description":"test plugin"}}"#
                ),
            )
            .unwrap();

            entries.push(format!(r#"{{"name":"{name}","source":"{name}"}}"#));
        }

        let plugins_json = entries.join(",");
        let mp_json = format!(
            r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{plugins_json}]}}"#
        );
        fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
        load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap()
    }

    #[test]
    fn remove_existing_plugin() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta"]);

        let removed = remove_plugins(
            &["alpha".to_string()],
            false,
            &config,
        )
        .unwrap();

        assert_eq!(removed, vec!["alpha"]);

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].name, "beta");
        assert_eq!(mp.version, "0.1.1");

        // Plugin directory should still exist
        assert!(config.plugin_root_abs.join("alpha").exists());
    }

    #[test]
    fn remove_nonexistent_plugin_returns_error() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let result = remove_plugins(
            &["nonexistent".to_string()],
            false,
            &config,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            SoukError::PluginNotFound(name) => assert_eq!(name, "nonexistent"),
            other => panic!("Expected PluginNotFound, got: {other}"),
        }
    }

    #[test]
    fn remove_with_delete_removes_directory() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta"]);

        assert!(config.plugin_root_abs.join("alpha").exists());

        let removed = remove_plugins(
            &["alpha".to_string()],
            true, // delete files
            &config,
        )
        .unwrap();

        assert_eq!(removed, vec!["alpha"]);

        // Plugin directory should be gone
        assert!(!config.plugin_root_abs.join("alpha").exists());
        // Other plugin should still exist
        assert!(config.plugin_root_abs.join("beta").exists());
    }

    #[test]
    fn remove_without_delete_keeps_directory() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let removed = remove_plugins(
            &["alpha".to_string()],
            false, // don't delete files
            &config,
        )
        .unwrap();

        assert_eq!(removed, vec!["alpha"]);

        // Plugin directory should still exist
        assert!(config.plugin_root_abs.join("alpha").exists());
    }

    #[test]
    fn remove_multiple_plugins() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta", "gamma"]);

        let removed = remove_plugins(
            &["alpha".to_string(), "gamma".to_string()],
            false,
            &config,
        )
        .unwrap();

        assert_eq!(removed.len(), 2);

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].name, "beta");
    }

    #[test]
    fn remove_empty_list_is_noop() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let removed = remove_plugins(&[], false, &config).unwrap();
        assert!(removed.is_empty());

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
    }
}
