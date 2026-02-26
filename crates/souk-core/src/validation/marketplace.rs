use std::collections::HashSet;
use std::path::Path;

use crate::discovery::MarketplaceConfig;
use crate::error::{ValidationDiagnostic, ValidationResult};
use crate::validation::plugin::validate_plugin;

/// Validates a marketplace configuration and optionally its plugins.
///
/// Checks that:
/// - The marketplace version is valid semver
/// - The plugin root directory exists
/// - There are no duplicate plugin names
/// - Each plugin entry has a non-empty name and source
/// - Filesystem completeness: every directory in the plugin root is listed
///   in the marketplace, and every marketplace entry has a corresponding directory
/// - If `skip_plugins` is false, each plugin is individually validated
pub fn validate_marketplace(config: &MarketplaceConfig, skip_plugins: bool) -> ValidationResult {
    let mut result = ValidationResult::new();
    let mp = &config.marketplace;

    if semver::Version::parse(&mp.version).is_err() {
        result.push(
            ValidationDiagnostic::error(format!("Invalid marketplace version: {}", mp.version))
                .with_path(&config.marketplace_path)
                .with_field("version"),
        );
    }

    if !config.plugin_root_abs.is_dir() {
        result.push(
            ValidationDiagnostic::error(format!(
                "Plugin root directory not found: {}",
                config.plugin_root_abs.display()
            ))
            .with_path(&config.marketplace_path)
            .with_field("pluginRoot"),
        );
    }

    let mut seen_names = HashSet::new();
    for entry in &mp.plugins {
        if !seen_names.insert(&entry.name) {
            result.push(
                ValidationDiagnostic::error(format!("Duplicate plugin name: {}", entry.name))
                    .with_path(&config.marketplace_path),
            );
        }
    }

    for (i, entry) in mp.plugins.iter().enumerate() {
        if entry.name.is_empty() {
            result.push(
                ValidationDiagnostic::error(format!("Plugin entry {i} has empty name"))
                    .with_path(&config.marketplace_path)
                    .with_field(format!("plugins[{i}].name")),
            );
        }
        if entry.source.is_empty() {
            result.push(
                ValidationDiagnostic::error(format!("Plugin entry {i} has empty source"))
                    .with_path(&config.marketplace_path)
                    .with_field(format!("plugins[{i}].source")),
            );
        }
    }

    if config.plugin_root_abs.is_dir() {
        let completeness = check_completeness(config);
        result.merge(completeness);
    }

    if !skip_plugins && config.plugin_root_abs.is_dir() {
        for entry in &mp.plugins {
            let source = &entry.source;
            let plugin_path = crate::resolution::resolve_source(source, config)
                .unwrap_or_else(|_| config.plugin_root_abs.join(source));

            if plugin_path.is_dir() {
                let plugin_result = validate_plugin(&plugin_path);
                result.merge(plugin_result);
            }
        }
    }

    result
}

/// Checks that the filesystem and marketplace are in sync.
///
/// Reports:
/// - A warning for each directory in the plugin root that is not listed
///   in the marketplace
/// - An error for each marketplace entry whose source directory does not
///   exist on the filesystem
fn check_completeness(config: &MarketplaceConfig) -> ValidationResult {
    let mut result = ValidationResult::new();

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

    for fs_name in &fs_plugins {
        if !mp_sources.contains(fs_name) {
            result.push(
                ValidationDiagnostic::warning(format!(
                    "Plugin in filesystem but not in marketplace: {fs_name}"
                ))
                .with_path(config.plugin_root_abs.join(fs_name)),
            );
        }
    }

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
    fn valid_marketplace() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"a","source":"a"}]}"#,
            &["a"],
        );
        let result = validate_marketplace(&config, false);
        assert!(
            !result.has_errors(),
            "diagnostics: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn invalid_version() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"bad","pluginRoot":"./plugins","plugins":[]}"#,
            &[],
        );
        let result = validate_marketplace(&config, true);
        assert!(result.has_errors());
    }

    #[test]
    fn duplicate_names() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[
                {"name":"a","source":"a"},{"name":"a","source":"b"}
            ]}"#,
            &["a", "b"],
        );
        let result = validate_marketplace(&config, true);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Duplicate")));
    }

    #[test]
    fn completeness_filesystem_not_in_marketplace() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[]}"#,
            &["orphan"],
        );
        let result = validate_marketplace(&config, true);
        assert!(result.warning_count() > 0);
    }

    #[test]
    fn completeness_marketplace_not_in_filesystem() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"missing","source":"missing"}]}"#,
            &[],
        );
        let result = validate_marketplace(&config, true);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("not in filesystem")));
    }

    #[test]
    fn skip_plugins_skips_individual_validation() {
        let tmp = TempDir::new().unwrap();
        let plugins = tmp.path().join("plugins");
        let bad = plugins.join("bad").join(".claude-plugin");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("plugin.json"), "not json").unwrap();

        let claude = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join("marketplace.json"),
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{"name":"bad","source":"bad"}]}"#,
        )
        .unwrap();

        let config = load_marketplace_config(&claude.join("marketplace.json")).unwrap();
        let result = validate_marketplace(&config, true);
        assert!(!result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Invalid JSON in plugin")));
    }

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

    #[test]
    fn empty_marketplace_is_valid() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"version":"0.1.0","pluginRoot":"./plugins","plugins":[]}"#,
            &[],
        );
        let result = validate_marketplace(&config, true);
        assert!(!result.has_errors());
    }
}
