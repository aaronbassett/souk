use std::path::{Path, PathBuf};

use crate::error::SoukError;
use crate::types::marketplace::Marketplace;

#[derive(Debug, Clone)]
pub struct MarketplaceConfig {
    pub marketplace_path: PathBuf,
    pub project_root: PathBuf,
    pub plugin_root_abs: PathBuf,
    pub marketplace: Marketplace,
}

pub fn discover_marketplace(start_dir: &Path) -> Result<PathBuf, SoukError> {
    let mut current = start_dir
        .canonicalize()
        .map_err(|e| SoukError::Io(e))?;

    loop {
        let candidate = current.join(".claude-plugin").join("marketplace.json");
        if candidate.is_file() {
            return Ok(candidate);
        }

        if current.join(".git").exists() {
            break;
        }

        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => break,
        }
    }

    Err(SoukError::MarketplaceNotFound(start_dir.to_path_buf()))
}

pub fn load_marketplace_config(marketplace_path: &Path) -> Result<MarketplaceConfig, SoukError> {
    let marketplace_path = marketplace_path
        .canonicalize()
        .map_err(|e| SoukError::Io(e))?;

    let content = std::fs::read_to_string(&marketplace_path)?;
    let marketplace: Marketplace = serde_json::from_str(&content)?;

    let claude_plugin_dir = marketplace_path
        .parent()
        .ok_or_else(|| SoukError::Other("Invalid marketplace path".into()))?;
    let project_root = claude_plugin_dir
        .parent()
        .ok_or_else(|| SoukError::Other("Invalid marketplace path".into()))?
        .to_path_buf();

    let plugin_root_rel = marketplace.normalized_plugin_root();
    let plugin_root_abs = project_root.join(&plugin_root_rel).canonicalize().map_err(
        |_| {
            SoukError::Other(format!(
                "Plugin root directory not found: {}",
                project_root.join(&plugin_root_rel).display()
            ))
        },
    )?;

    Ok(MarketplaceConfig {
        marketplace_path,
        project_root,
        plugin_root_abs,
        marketplace,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_marketplace(tmp: &TempDir) -> PathBuf {
        let claude_dir = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let mp_path = claude_dir.join("marketplace.json");
        std::fs::write(
            &mp_path,
            r#"{"version": "0.1.0", "pluginRoot": "./plugins", "plugins": []}"#,
        )
        .unwrap();
        mp_path
    }

    #[test]
    fn discover_from_project_root() {
        let tmp = TempDir::new().unwrap();
        let mp_path = setup_marketplace(&tmp);
        let found = discover_marketplace(tmp.path()).unwrap();
        assert_eq!(found, mp_path.canonicalize().unwrap());
    }

    #[test]
    fn discover_from_subdirectory() {
        let tmp = TempDir::new().unwrap();
        setup_marketplace(&tmp);
        let sub = tmp.path().join("plugins").join("my-plugin");
        std::fs::create_dir_all(&sub).unwrap();

        let found = discover_marketplace(&sub).unwrap();
        assert!(found.ends_with("marketplace.json"));
    }

    #[test]
    fn discover_not_found() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        let result = discover_marketplace(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn load_marketplace_config_resolves_paths() {
        let tmp = TempDir::new().unwrap();
        let mp_path = setup_marketplace(&tmp);
        let config = load_marketplace_config(&mp_path).unwrap();
        assert_eq!(config.project_root, tmp.path().canonicalize().unwrap());
        assert!(config.plugin_root_abs.ends_with("plugins"));
    }

    #[test]
    fn load_marketplace_config_default_plugin_root() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        let mp_path = claude_dir.join("marketplace.json");
        std::fs::write(&mp_path, r#"{"version": "0.1.0", "plugins": []}"#).unwrap();

        let config = load_marketplace_config(&mp_path).unwrap();
        assert!(config.plugin_root_abs.ends_with("plugins"));
    }
}
