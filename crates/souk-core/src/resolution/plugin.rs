use std::path::{Path, PathBuf};

use crate::discovery::MarketplaceConfig;
use crate::error::SoukError;

pub fn resolve_plugin(
    input: &str,
    config: Option<&MarketplaceConfig>,
) -> Result<PathBuf, SoukError> {
    let direct = PathBuf::from(input);
    if direct.is_dir() {
        return direct.canonicalize().map_err(|e| SoukError::Io(e));
    }

    if let Some(config) = config {
        let relative = config.plugin_root_abs.join(input);
        if relative.is_dir() {
            return relative.canonicalize().map_err(|e| SoukError::Io(e));
        }

        if let Some(entry) = config
            .marketplace
            .plugins
            .iter()
            .find(|p| p.name == input)
        {
            let resolved = resolve_source(&entry.source, config)?;
            if resolved.is_dir() {
                return resolved.canonicalize().map_err(|e| SoukError::Io(e));
            }
        }
    }

    Err(SoukError::PluginNotFound(input.to_string()))
}

pub fn resolve_source(
    source: &str,
    config: &MarketplaceConfig,
) -> Result<PathBuf, SoukError> {
    if source.starts_with('/') {
        Ok(PathBuf::from(source))
    } else if source.starts_with("./") || source.starts_with("../") {
        Ok(config.project_root.join(source))
    } else {
        Ok(config.plugin_root_abs.join(source))
    }
}

pub fn plugin_path_to_source(
    path: &Path,
    config: &MarketplaceConfig,
) -> (String, bool) {
    let canon_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let canon_root = &config.plugin_root_abs;

    if let Ok(relative) = canon_path.strip_prefix(canon_root) {
        let dir_name = relative
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        (dir_name, true)
    } else {
        (canon_path.to_string_lossy().to_string(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::load_marketplace_config;
    use tempfile::TempDir;

    fn setup(tmp: &TempDir) -> MarketplaceConfig {
        let claude_dir = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let plugin_dir = plugins_dir.join("my-plugin").join(".claude-plugin");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "my-plugin", "version": "1.0.0", "description": "test"}"#,
        )
        .unwrap();

        let mp_path = claude_dir.join("marketplace.json");
        std::fs::write(
            &mp_path,
            r#"{
                "version": "0.1.0",
                "pluginRoot": "./plugins",
                "plugins": [
                    {"name": "My Plugin", "source": "my-plugin"}
                ]
            }"#,
        )
        .unwrap();

        load_marketplace_config(&mp_path).unwrap()
    }

    #[test]
    fn resolve_by_direct_path() {
        let tmp = TempDir::new().unwrap();
        let config = setup(&tmp);
        let plugin_path = tmp.path().join("plugins").join("my-plugin");
        let result = resolve_plugin(plugin_path.to_str().unwrap(), Some(&config));
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_by_plugin_root_relative() {
        let tmp = TempDir::new().unwrap();
        let config = setup(&tmp);
        let result = resolve_plugin("my-plugin", Some(&config));
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_by_marketplace_name() {
        let tmp = TempDir::new().unwrap();
        let config = setup(&tmp);
        let result = resolve_plugin("My Plugin", Some(&config));
        assert!(result.is_ok());
    }

    #[test]
    fn resolve_not_found() {
        let tmp = TempDir::new().unwrap();
        let config = setup(&tmp);
        let result = resolve_plugin("nonexistent", Some(&config));
        assert!(matches!(result, Err(SoukError::PluginNotFound(_))));
    }

    #[test]
    fn path_to_source_internal() {
        let tmp = TempDir::new().unwrap();
        let config = setup(&tmp);
        let path = config.plugin_root_abs.join("my-plugin");
        let (source, is_internal) = plugin_path_to_source(&path, &config);
        assert_eq!(source, "my-plugin");
        assert!(is_internal);
    }

    #[test]
    fn path_to_source_external() {
        let tmp = TempDir::new().unwrap();
        let config = setup(&tmp);
        let external = TempDir::new().unwrap();
        let (source, is_internal) =
            plugin_path_to_source(external.path(), &config);
        assert!(!is_internal);
        assert!(source.starts_with('/'));
    }
}
