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

/// The result of a remove operation.
#[derive(Debug)]
pub struct RemoveResult {
    /// Plugin names that were successfully removed from the marketplace.
    pub removed: Vec<String>,
    /// Non-fatal warnings (e.g., directory delete failures).
    pub warnings: Vec<String>,
}

/// Removes the named plugins from the marketplace.
///
/// For each name in `names`:
/// - Finds the matching entry in marketplace.json
/// - If `delete_files` is true, also removes the plugin directory from disk
/// - Bumps the marketplace version (patch)
///
/// Returns a [`RemoveResult`] with the removed names and any warnings
/// (e.g., if a directory could not be deleted after the marketplace entry
/// was removed).
///
/// # Errors
///
/// Returns [`SoukError::PluginNotFound`] if any name does not exist in
/// the marketplace.
///
/// Returns [`SoukError::AtomicRollback`] if the post-removal validation fails.
///
/// # Example
///
/// ```no_run
/// # use souk_core::ops::remove::remove_plugins;
/// # fn example(config: &souk_core::discovery::MarketplaceConfig) {
/// let result = remove_plugins(
///     &["my-plugin".to_string()],
///     true,  // delete files
///     false, // don't allow external deletes
///     config,
/// ).unwrap();
///
/// for name in &result.removed {
///     println!("Removed: {name}");
/// }
/// for warn in &result.warnings {
///     eprintln!("Warning: {warn}");
/// }
/// # }
pub fn remove_plugins(
    names: &[String],
    delete_files: bool,
    allow_external_delete: bool,
    config: &MarketplaceConfig,
) -> Result<RemoveResult, SoukError> {
    if names.is_empty() {
        return Ok(RemoveResult {
            removed: Vec::new(),
            warnings: Vec::new(),
        });
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
        let plugin_root = config
            .plugin_root_abs
            .canonicalize()
            .map_err(SoukError::Io)?;

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
    let mut warnings = Vec::new();
    for (name, path) in &delete_targets {
        if path.is_dir() {
            if let Err(e) = fs::remove_dir_all(path) {
                warnings.push(format!(
                    "Removed '{name}' from marketplace but failed to delete directory {}: {e}",
                    path.display()
                ));
            }
        }
    }

    Ok(RemoveResult { removed, warnings })
}

/// Deletes a plugin directory from disk. Exposed for testing or direct use.
pub fn delete_plugin_dir(
    source: &str,
    allow_external_delete: bool,
    config: &MarketplaceConfig,
) -> Result<(), SoukError> {
    let plugin_path = resolve_source(source, config)?;
    if plugin_path.is_dir() {
        let resolved = plugin_path.canonicalize().map_err(SoukError::Io)?;
        let plugin_root = config
            .plugin_root_abs
            .canonicalize()
            .map_err(SoukError::Io)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::load_marketplace_config;
    use tempfile::TempDir;

    fn setup_marketplace_with_plugins(tmp: &TempDir, plugin_names: &[&str]) -> MarketplaceConfig {
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
                format!(r#"{{"name":"{name}","version":"1.0.0","description":"test plugin"}}"#),
            )
            .unwrap();

            entries.push(format!(r#"{{"name":"{name}","source":"{name}"}}"#));
        }

        let plugins_json = entries.join(",");
        let mp_json =
            format!(r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{plugins_json}]}}"#);
        fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
        load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap()
    }

    #[test]
    fn remove_existing_plugin() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta"]);

        let result = remove_plugins(&["alpha".to_string()], false, false, &config).unwrap();

        assert_eq!(result.removed, vec!["alpha"]);
        assert!(result.warnings.is_empty());

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

        let result = remove_plugins(&["nonexistent".to_string()], false, false, &config);

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

        let result = remove_plugins(
            &["alpha".to_string()],
            true, // delete files
            false,
            &config,
        )
        .unwrap();

        assert_eq!(result.removed, vec!["alpha"]);
        assert!(result.warnings.is_empty());

        // Plugin directory should be gone
        assert!(!config.plugin_root_abs.join("alpha").exists());
        // Other plugin should still exist
        assert!(config.plugin_root_abs.join("beta").exists());
    }

    #[test]
    fn remove_without_delete_keeps_directory() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let result = remove_plugins(
            &["alpha".to_string()],
            false, // don't delete files
            false,
            &config,
        )
        .unwrap();

        assert_eq!(result.removed, vec!["alpha"]);

        // Plugin directory should still exist
        assert!(config.plugin_root_abs.join("alpha").exists());
    }

    #[test]
    fn remove_multiple_plugins() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta", "gamma"]);

        let result = remove_plugins(
            &["alpha".to_string(), "gamma".to_string()],
            false,
            false,
            &config,
        )
        .unwrap();

        assert_eq!(result.removed.len(), 2);

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].name, "beta");
    }

    #[test]
    fn remove_empty_list_is_noop() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let result = remove_plugins(&[], false, false, &config).unwrap();
        assert!(result.removed.is_empty());

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
    }

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

        let ext_path_str = ext_plugin.to_string_lossy().replace('\\', "/");
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

        let ext_path_str = ext_plugin.to_string_lossy().replace('\\', "/");
        let mp_json = format!(
            r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{{"name":"ext","source":"{ext_path_str}"}}]}}"#
        );
        fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
        let config = load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap();

        // Delete with allow flag — should succeed
        let result = remove_plugins(&["ext".to_string()], true, true, &config).unwrap();
        assert_eq!(result.removed, vec!["ext"]);
        assert!(!ext_plugin.exists());
    }

    #[test]
    fn remove_internal_plugin_delete_works_without_flag() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        assert!(config.plugin_root_abs.join("alpha").exists());

        let result = remove_plugins(&["alpha".to_string()], true, false, &config).unwrap();
        assert_eq!(result.removed, vec!["alpha"]);
        assert!(!config.plugin_root_abs.join("alpha").exists());
    }
}
