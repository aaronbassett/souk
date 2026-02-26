//! Update plugin metadata in the marketplace.
//!
//! Re-reads plugin.json from disk to refresh the marketplace entry, and
//! optionally bumps the plugin version.

use std::fs;

use crate::discovery::{load_marketplace_config, MarketplaceConfig};
use crate::error::SoukError;
use crate::ops::AtomicGuard;
use crate::resolution::resolve_source;
use crate::types::{Marketplace, PluginManifest};
use crate::validation::{validate_marketplace, validate_plugin};
use crate::version::{bump_major, bump_minor, bump_patch};

/// Updates the named plugins in the marketplace by re-reading their
/// plugin.json from disk.
///
/// For each name in `names`:
/// - Resolves the plugin to its directory via the marketplace source
/// - Re-reads plugin.json
/// - Updates the marketplace entry (name, tags)
/// - If `bump_type` is specified ("major", "minor", or "patch"), bumps
///   the version in the plugin's plugin.json file
/// - Re-validates the plugin after update
///
/// The marketplace version is always bumped (patch) at the end.
///
/// # Errors
///
/// Returns [`SoukError::PluginNotFound`] if any name does not exist in
/// the marketplace.
///
/// Returns [`SoukError::AtomicRollback`] if post-update validation fails.
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

    // If a version bump is requested, first update plugin.json files on disk
    if let Some(bump) = bump_type {
        for name in names {
            let entry = config
                .marketplace
                .plugins
                .iter()
                .find(|p| p.name == *name)
                .unwrap();

            let plugin_path = resolve_source(&entry.source, config)?;
            let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");

            let content = fs::read_to_string(&plugin_json_path).map_err(|e| {
                SoukError::Other(format!("Cannot read plugin.json for {name}: {e}"))
            })?;

            // Parse as generic JSON to preserve all fields
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

    // Atomic update on marketplace.json
    let guard = AtomicGuard::new(&config.marketplace_path)?;

    let content = fs::read_to_string(&config.marketplace_path)?;
    let mut marketplace: Marketplace = serde_json::from_str(&content)?;

    let mut updated = Vec::new();

    for name in names {
        let entry = marketplace
            .plugins
            .iter()
            .find(|p| p.name == *name)
            .unwrap();
        let source = entry.source.clone();

        // Re-read plugin.json to refresh metadata
        let plugin_path = resolve_source(&source, config)?;
        let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");

        let pj_content = fs::read_to_string(&plugin_json_path)
            .map_err(|e| SoukError::Other(format!("Cannot read plugin.json for {name}: {e}")))?;

        let manifest: PluginManifest = serde_json::from_str(&pj_content)?;

        // Update marketplace entry
        if let Some(entry) = marketplace.plugins.iter_mut().find(|p| p.name == *name) {
            entry.tags = manifest.keywords.clone();
            // If plugin.json has a different name, update it
            if let Some(new_name) = manifest.name_str() {
                if new_name != name {
                    entry.name = new_name.to_string();
                }
            }
        }

        // Re-validate the plugin
        let validation = validate_plugin(&plugin_path);
        if validation.has_errors() {
            drop(guard);
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
        drop(guard);
        return Err(SoukError::AtomicRollback(
            "Marketplace validation failed after update".to_string(),
        ));
    }

    guard.commit()?;

    Ok(updated)
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
            let plugin_dir = plugins_dir.join(name);
            let plugin_claude = plugin_dir.join(".claude-plugin");
            fs::create_dir_all(&plugin_claude).unwrap();
            fs::write(
                plugin_claude.join("plugin.json"),
                format!(
                    r#"{{"name":"{name}","version":"1.0.0","description":"test plugin","keywords":["original"]}}"#
                ),
            )
            .unwrap();

            entries.push(format!(
                r#"{{"name":"{name}","source":"{name}","tags":["old"]}}"#
            ));
        }

        let plugins_json = entries.join(",");
        let mp_json =
            format!(r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{plugins_json}]}}"#);
        fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
        load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap()
    }

    #[test]
    fn update_refreshes_metadata_from_disk() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        // Verify initial tags are "old"
        assert_eq!(config.marketplace.plugins[0].tags, vec!["old"]);

        // Update should refresh tags from plugin.json (which has "original")
        let updated = update_plugins(&["alpha".to_string()], None, &config).unwrap();

        assert_eq!(updated, vec!["alpha"]);

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins[0].tags, vec!["original"]);
        assert_eq!(mp.version, "0.1.1");
    }

    #[test]
    fn update_with_patch_bumps_version() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let updated = update_plugins(&["alpha".to_string()], Some("patch"), &config).unwrap();

        assert_eq!(updated, vec!["alpha"]);

        // Check plugin.json version was bumped
        let plugin_json_path = config
            .plugin_root_abs
            .join("alpha")
            .join(".claude-plugin")
            .join("plugin.json");
        let content = fs::read_to_string(&plugin_json_path).unwrap();
        let manifest: PluginManifest = serde_json::from_str(&content).unwrap();
        assert_eq!(manifest.version_str(), Some("1.0.1"));
    }

    #[test]
    fn update_with_major_bumps_version() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        update_plugins(&["alpha".to_string()], Some("major"), &config).unwrap();

        let plugin_json_path = config
            .plugin_root_abs
            .join("alpha")
            .join(".claude-plugin")
            .join("plugin.json");
        let content = fs::read_to_string(&plugin_json_path).unwrap();
        let manifest: PluginManifest = serde_json::from_str(&content).unwrap();
        assert_eq!(manifest.version_str(), Some("2.0.0"));
    }

    #[test]
    fn update_with_minor_bumps_version() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        update_plugins(&["alpha".to_string()], Some("minor"), &config).unwrap();

        let plugin_json_path = config
            .plugin_root_abs
            .join("alpha")
            .join(".claude-plugin")
            .join("plugin.json");
        let content = fs::read_to_string(&plugin_json_path).unwrap();
        let manifest: PluginManifest = serde_json::from_str(&content).unwrap();
        assert_eq!(manifest.version_str(), Some("1.1.0"));
    }

    #[test]
    fn update_nonexistent_plugin_returns_error() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha"]);

        let result = update_plugins(&["nonexistent".to_string()], None, &config);

        assert!(result.is_err());
        match result.unwrap_err() {
            SoukError::PluginNotFound(name) => assert_eq!(name, "nonexistent"),
            other => panic!("Expected PluginNotFound, got: {other}"),
        }
    }

    #[test]
    fn update_multiple_plugins() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_with_plugins(&tmp, &["alpha", "beta"]);

        let updated = update_plugins(
            &["alpha".to_string(), "beta".to_string()],
            Some("patch"),
            &config,
        )
        .unwrap();

        assert_eq!(updated.len(), 2);

        // Both plugins should have bumped versions
        for name in &["alpha", "beta"] {
            let plugin_json_path = config
                .plugin_root_abs
                .join(name)
                .join(".claude-plugin")
                .join("plugin.json");
            let content = fs::read_to_string(&plugin_json_path).unwrap();
            let manifest: PluginManifest = serde_json::from_str(&content).unwrap();
            assert_eq!(manifest.version_str(), Some("1.0.1"));
        }
    }
}
