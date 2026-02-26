use std::path::Path;

use crate::error::{ValidationDiagnostic, ValidationResult};
use crate::types::plugin::PluginManifest;
use crate::validation::extends::validate_extends_plugin;

/// Validates a plugin directory.
///
/// Checks that:
/// - The path exists and is a directory
/// - It contains a `.claude-plugin/` subdirectory
/// - The `.claude-plugin/plugin.json` file exists and is valid JSON
/// - Required fields (`name`, `version`, `description`) are present and non-null
/// - The `version` field is valid semver
/// - If an `extends-plugin.json` exists, it is also validated
pub fn validate_plugin(plugin_path: &Path) -> ValidationResult {
    let mut result = ValidationResult::new();

    if !plugin_path.is_dir() {
        result.push(
            ValidationDiagnostic::error(format!(
                "Plugin path does not exist or is not a directory: {}",
                plugin_path.display()
            ))
            .with_path(plugin_path),
        );
        return result;
    }

    let claude_dir = plugin_path.join(".claude-plugin");

    if !claude_dir.is_dir() {
        result.push(
            ValidationDiagnostic::error("Missing .claude-plugin directory")
                .with_path(plugin_path),
        );
        return result;
    }

    let plugin_json_path = claude_dir.join("plugin.json");

    if !plugin_json_path.is_file() {
        result.push(
            ValidationDiagnostic::error("Missing plugin.json").with_path(&claude_dir),
        );
        return result;
    }

    let content = match std::fs::read_to_string(&plugin_json_path) {
        Ok(c) => c,
        Err(e) => {
            result.push(
                ValidationDiagnostic::error(format!("Cannot read plugin.json: {e}"))
                    .with_path(&plugin_json_path),
            );
            return result;
        }
    };

    let manifest: PluginManifest = match serde_json::from_str(&content) {
        Ok(m) => m,
        Err(e) => {
            result.push(
                ValidationDiagnostic::error(format!("Invalid JSON in plugin.json: {e}"))
                    .with_path(&plugin_json_path),
            );
            return result;
        }
    };

    if manifest.name_str().is_none() {
        result.push(
            ValidationDiagnostic::error("Missing or null required field: name")
                .with_path(&plugin_json_path)
                .with_field("name"),
        );
    }

    let version_str = manifest.version_str();
    if version_str.is_none() {
        result.push(
            ValidationDiagnostic::error("Missing or null required field: version")
                .with_path(&plugin_json_path)
                .with_field("version"),
        );
    }

    if manifest.description_str().is_none() {
        result.push(
            ValidationDiagnostic::error("Missing or null required field: description")
                .with_path(&plugin_json_path)
                .with_field("description"),
        );
    }

    if let Some(v) = version_str {
        if semver::Version::parse(v).is_err() {
            result.push(
                ValidationDiagnostic::error(format!("Invalid semver version: {v}"))
                    .with_path(&plugin_json_path)
                    .with_field("version"),
            );
        }
    }

    let extends_result = validate_extends_plugin(plugin_path);
    result.merge(extends_result);

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_valid_plugin(tmp: &TempDir) -> std::path::PathBuf {
        let plugin = tmp.path().join("good-plugin");
        let claude = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join("plugin.json"),
            r#"{"name": "good-plugin", "version": "1.0.0", "description": "A good plugin"}"#,
        )
        .unwrap();
        plugin
    }

    #[test]
    fn valid_plugin_passes() {
        let tmp = TempDir::new().unwrap();
        let plugin = make_valid_plugin(&tmp);
        let result = validate_plugin(&plugin);
        assert!(
            !result.has_errors(),
            "diagnostics: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn nonexistent_path() {
        let result = validate_plugin(Path::new("/tmp/nonexistent-plugin-xyz"));
        assert!(result.has_errors());
        assert!(result.diagnostics[0].message.contains("does not exist"));
    }

    #[test]
    fn missing_claude_plugin_dir() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("bare-dir");
        std::fs::create_dir_all(&plugin).unwrap();
        let result = validate_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result.diagnostics[0].message.contains(".claude-plugin"));
    }

    #[test]
    fn missing_plugin_json() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("no-json");
        std::fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        let result = validate_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result.diagnostics[0].message.contains("plugin.json"));
    }

    #[test]
    fn invalid_json() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("bad-json");
        let claude = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("plugin.json"), "not json").unwrap();
        let result = validate_plugin(&plugin);
        assert!(result.has_errors());
    }

    #[test]
    fn missing_required_fields() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("empty-fields");
        let claude = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("plugin.json"), r#"{}"#).unwrap();
        let result = validate_plugin(&plugin);
        assert_eq!(result.error_count(), 3);
    }

    #[test]
    fn null_name() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("null-name");
        let claude = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join("plugin.json"),
            r#"{"name": null, "version": "1.0.0", "description": "desc"}"#,
        )
        .unwrap();
        let result = validate_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.field.as_deref() == Some("name")));
    }

    #[test]
    fn invalid_semver() {
        let tmp = TempDir::new().unwrap();
        let plugin = tmp.path().join("bad-version");
        let claude = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join("plugin.json"),
            r#"{"name": "test", "version": "not.semver", "description": "desc"}"#,
        )
        .unwrap();
        let result = validate_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("semver")));
    }

    #[test]
    fn valid_plugin_with_extends() {
        let tmp = TempDir::new().unwrap();
        let plugin = make_valid_plugin(&tmp);
        std::fs::write(
            plugin.join(".claude-plugin").join("extends-plugin.json"),
            r#"{"dependencies": {"foo": "^1.0.0"}}"#,
        )
        .unwrap();
        let result = validate_plugin(&plugin);
        assert!(!result.has_errors());
    }
}
