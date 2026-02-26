use serde::{Deserialize, Serialize};

/// A plugin entry in marketplace.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    pub name: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

/// The marketplace.json root document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Marketplace {
    pub version: String,
    #[serde(default = "default_plugin_root", skip_serializing_if = "Option::is_none")]
    pub plugin_root: Option<String>,
    pub plugins: Vec<PluginEntry>,
}

fn default_plugin_root() -> Option<String> {
    Some("./plugins".to_string())
}

impl Marketplace {
    pub fn plugin_root(&self) -> &str {
        self.plugin_root.as_deref().unwrap_or("./plugins")
    }

    pub fn normalized_plugin_root(&self) -> String {
        let root = self.plugin_root();
        if root.starts_with("./") || root.starts_with('/') {
            root.to_string()
        } else {
            format!("./{root}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_marketplace_json() {
        let json = r#"{
            "version": "0.1.0",
            "pluginRoot": "./plugins",
            "plugins": [
                {"name": "my-plugin", "source": "my-plugin", "tags": ["dev"]}
            ]
        }"#;
        let mp: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(mp.version, "0.1.0");
        assert_eq!(mp.plugin_root(), "./plugins");
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].name, "my-plugin");
        assert_eq!(mp.plugins[0].tags, vec!["dev"]);
    }

    #[test]
    fn default_plugin_root_when_missing() {
        let json = r#"{"version": "0.1.0", "plugins": []}"#;
        let mp: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(mp.plugin_root(), "./plugins");
    }

    #[test]
    fn normalize_plugin_root_without_dot_slash() {
        let json = r#"{"version": "0.1.0", "pluginRoot": "plugins", "plugins": []}"#;
        let mp: Marketplace = serde_json::from_str(json).unwrap();
        assert_eq!(mp.normalized_plugin_root(), "./plugins");
    }

    #[test]
    fn serialize_round_trip() {
        let mp = Marketplace {
            version: "1.0.0".to_string(),
            plugin_root: Some("./plugins".to_string()),
            plugins: vec![PluginEntry {
                name: "test".to_string(),
                source: "test".to_string(),
                tags: vec![],
            }],
        };
        let json = serde_json::to_string_pretty(&mp).unwrap();
        let mp2: Marketplace = serde_json::from_str(&json).unwrap();
        assert_eq!(mp2.version, "1.0.0");
    }
}
