//! Scaffolding logic for creating a new marketplace directory structure.
//!
//! This module implements the `souk init` core operation, which creates the
//! `.claude-plugin/` directory, writes a default `marketplace.json`, and
//! creates the plugin root directory.

use std::fs;
use std::path::Path;

use crate::error::SoukError;
use crate::types::Marketplace;

/// Scaffold a new marketplace at the given path.
///
/// Creates the `.claude-plugin/` directory with a `marketplace.json` file
/// and an empty plugin root directory. Returns an error if a marketplace
/// already exists at the target path.
///
/// # Arguments
///
/// * `path` - The root directory where the marketplace should be created.
/// * `plugin_root` - The relative path for the plugin root directory
///   (e.g., `"./plugins"`).
///
/// # Errors
///
/// Returns [`SoukError::MarketplaceAlreadyExists`] if
/// `.claude-plugin/marketplace.json` already exists at `path`.
///
/// Returns [`SoukError::Io`] if directory creation or file writing fails.
pub fn scaffold_marketplace(path: &Path, plugin_root: &str) -> Result<(), SoukError> {
    let claude_plugin_dir = path.join(".claude-plugin");
    let marketplace_path = claude_plugin_dir.join("marketplace.json");

    if marketplace_path.exists() {
        return Err(SoukError::MarketplaceAlreadyExists(marketplace_path));
    }

    // Create .claude-plugin/ directory (and any parent directories)
    fs::create_dir_all(&claude_plugin_dir)?;

    // Build the marketplace document
    let marketplace = Marketplace {
        version: "0.1.0".to_string(),
        plugin_root: Some(plugin_root.to_string()),
        plugins: Vec::new(),
    };

    let json = serde_json::to_string_pretty(&marketplace)?;
    fs::write(&marketplace_path, format!("{json}\n"))?;

    // Create the plugin root directory, stripping any leading "./" for path joining
    let plugin_root_stripped = plugin_root.strip_prefix("./").unwrap_or(plugin_root);
    let plugin_root_path = path.join(plugin_root_stripped);
    fs::create_dir_all(&plugin_root_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_creates_marketplace_structure() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        scaffold_marketplace(root, "./plugins").unwrap();

        // .claude-plugin/ directory should exist
        assert!(root.join(".claude-plugin").is_dir());

        // marketplace.json should exist and be valid
        let mp_path = root.join(".claude-plugin").join("marketplace.json");
        assert!(mp_path.is_file());

        let contents = fs::read_to_string(&mp_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&contents).unwrap();
        assert_eq!(mp.version, "0.1.0");
        assert_eq!(mp.plugin_root(), "./plugins");
        assert!(mp.plugins.is_empty());

        // plugins/ directory should exist
        assert!(root.join("plugins").is_dir());
    }

    #[test]
    fn scaffold_returns_error_if_marketplace_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        // First init should succeed
        scaffold_marketplace(root, "./plugins").unwrap();

        // Second init should fail with MarketplaceAlreadyExists
        let result = scaffold_marketplace(root, "./plugins");
        assert!(result.is_err());

        match result.unwrap_err() {
            SoukError::MarketplaceAlreadyExists(path) => {
                assert!(path.ends_with(".claude-plugin/marketplace.json"));
            }
            other => panic!("Expected MarketplaceAlreadyExists, got: {other}"),
        }
    }

    #[test]
    fn scaffold_respects_custom_plugin_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        scaffold_marketplace(root, "./extensions").unwrap();

        // marketplace.json should reference the custom root
        let mp_path = root.join(".claude-plugin").join("marketplace.json");
        let contents = fs::read_to_string(&mp_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&contents).unwrap();
        assert_eq!(mp.plugin_root(), "./extensions");

        // extensions/ directory should exist
        assert!(root.join("extensions").is_dir());
    }

    #[test]
    fn scaffold_default_plugin_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        scaffold_marketplace(root, "./plugins").unwrap();

        let mp_path = root.join(".claude-plugin").join("marketplace.json");
        let contents = fs::read_to_string(&mp_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&contents).unwrap();
        assert_eq!(mp.plugin_root(), "./plugins");

        assert!(root.join("plugins").is_dir());
    }

    #[test]
    fn scaffold_creates_parent_directories_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("nested").join("deep").join("marketplace");

        scaffold_marketplace(&root, "./plugins").unwrap();

        assert!(root
            .join(".claude-plugin")
            .join("marketplace.json")
            .is_file());
        assert!(root.join("plugins").is_dir());
    }

    #[test]
    fn scaffold_plugin_root_without_dot_slash_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();

        scaffold_marketplace(root, "custom-plugins").unwrap();

        let mp_path = root.join(".claude-plugin").join("marketplace.json");
        let contents = fs::read_to_string(&mp_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&contents).unwrap();
        assert_eq!(mp.plugin_root(), "custom-plugins");

        // Directory should be created without the "./" prefix
        assert!(root.join("custom-plugins").is_dir());
    }
}
