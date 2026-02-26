use std::path::Path;

use crate::error::{ValidationDiagnostic, ValidationResult};
use crate::types::version_constraint::is_valid_version_constraint;

const ALLOWED_KEYS: &[&str] = &[
    "dependencies",
    "optionalDependencies",
    "systemDependencies",
    "optionalSystemDependencies",
];

/// Validates the `extends-plugin.json` file within a plugin directory.
///
/// This file allows a plugin to declare dependencies on other plugins or
/// system packages. Each section must be a JSON object mapping dependency
/// names to version constraints (either a string or an object with a
/// `version` field).
///
/// Returns an empty result if the file does not exist (it is optional).
pub fn validate_extends_plugin(plugin_path: &Path) -> ValidationResult {
    let mut result = ValidationResult::new();
    let extends_path = plugin_path
        .join(".claude-plugin")
        .join("extends-plugin.json");

    if !extends_path.is_file() {
        return result;
    }

    let content = match std::fs::read_to_string(&extends_path) {
        Ok(c) => c,
        Err(e) => {
            result.push(
                ValidationDiagnostic::error(format!("Cannot read extends-plugin.json: {e}"))
                    .with_path(&extends_path),
            );
            return result;
        }
    };

    let doc: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            result.push(
                ValidationDiagnostic::error(format!("Invalid JSON in extends-plugin.json: {e}"))
                    .with_path(&extends_path),
            );
            return result;
        }
    };

    let Some(obj) = doc.as_object() else {
        result.push(
            ValidationDiagnostic::error("extends-plugin.json must be a JSON object")
                .with_path(&extends_path),
        );
        return result;
    };

    for key in obj.keys() {
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            result.push(
                ValidationDiagnostic::error(format!("Invalid key in extends-plugin.json: {key}"))
                    .with_path(&extends_path)
                    .with_field(key.clone()),
            );
        }
    }

    for section_name in ALLOWED_KEYS {
        if let Some(section) = obj.get(*section_name) {
            if section.is_null() {
                continue;
            }
            let Some(section_obj) = section.as_object() else {
                result.push(
                    ValidationDiagnostic::error(format!(
                        "Invalid {section_name} in extends-plugin.json: expected object, got {}",
                        value_type_name(section)
                    ))
                    .with_path(&extends_path)
                    .with_field(section_name.to_string()),
                );
                continue;
            };

            for (dep_name, dep_value) in section_obj {
                let version = extract_version(dep_value);
                match version {
                    Some(v) => {
                        if !is_valid_version_constraint(&v) {
                            result.push(
                                ValidationDiagnostic::error(format!(
                                    "Invalid version constraint in {section_name}: {v} (for {dep_name})"
                                ))
                                .with_path(&extends_path)
                                .with_field(format!("{section_name}.{dep_name}")),
                            );
                        }
                    }
                    None => {
                        result.push(
                            ValidationDiagnostic::error(format!(
                                "Invalid dependency value in {section_name}: must be string or object with version (for {dep_name})"
                            ))
                            .with_path(&extends_path)
                            .with_field(format!("{section_name}.{dep_name}")),
                        );
                    }
                }
            }
        }
    }

    result
}

/// Extracts a version constraint string from a dependency value.
///
/// A dependency value can be:
/// - A string (the version constraint itself)
/// - An object with an optional `version` field (defaults to `"*"`)
/// - Anything else returns `None` (invalid)
fn extract_version(value: &serde_json::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        Some(s.to_string())
    } else {
        value.as_object().map(|obj| {
            obj.get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("*")
                .to_string()
        })
    }
}

fn value_type_name(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Null => "null",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_extends(tmp: &TempDir, content: &str) -> std::path::PathBuf {
        let plugin = tmp.path().join("test-plugin");
        let claude = plugin.join(".claude-plugin");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(claude.join("extends-plugin.json"), content).unwrap();
        plugin
    }

    #[test]
    fn valid_extends() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(
            &tmp,
            r#"{
            "dependencies": {"foo": "^1.0.0"},
            "optionalDependencies": {"bar": {"version": "~2.0.0"}},
            "systemDependencies": {"baz": "*"}
        }"#,
        );
        let result = validate_extends_plugin(&plugin);
        assert!(!result.has_errors());
    }

    #[test]
    fn missing_file_is_ok() {
        let tmp = TempDir::new().unwrap();
        let result = validate_extends_plugin(tmp.path());
        assert!(!result.has_errors());
    }

    #[test]
    fn invalid_json() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(&tmp, "not json");
        let result = validate_extends_plugin(&plugin);
        assert!(result.has_errors());
    }

    #[test]
    fn invalid_top_level_key() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(&tmp, r#"{"badKey": {}}"#);
        let result = validate_extends_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result.diagnostics[0].message.contains("Invalid key"));
    }

    #[test]
    fn section_must_be_object() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(&tmp, r#"{"dependencies": ["not", "an", "object"]}"#);
        let result = validate_extends_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result.diagnostics[0].message.contains("expected object"));
    }

    #[test]
    fn invalid_version_constraint() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(&tmp, r#"{"dependencies": {"foo": "latest"}}"#);
        let result = validate_extends_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result.diagnostics[0]
            .message
            .contains("Invalid version constraint"));
    }

    #[test]
    fn object_value_without_version_defaults_to_star() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(&tmp, r#"{"dependencies": {"foo": {"notes": "optional"}}}"#);
        let result = validate_extends_plugin(&plugin);
        assert!(!result.has_errors());
    }

    #[test]
    fn non_string_non_object_value() {
        let tmp = TempDir::new().unwrap();
        let plugin = write_extends(&tmp, r#"{"dependencies": {"foo": 42}}"#);
        let result = validate_extends_plugin(&plugin);
        assert!(result.has_errors());
        assert!(result.diagnostics[0]
            .message
            .contains("must be string or object"));
    }
}
