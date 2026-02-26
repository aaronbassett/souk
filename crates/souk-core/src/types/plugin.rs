use serde::{Deserialize, Serialize};

/// A plugin.json manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: Option<serde_json::Value>,
    pub version: Option<serde_json::Value>,
    pub description: Option<serde_json::Value>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

impl PluginManifest {
    pub fn name_str(&self) -> Option<&str> {
        self.name.as_ref().and_then(|v| v.as_str())
    }

    pub fn version_str(&self) -> Option<&str> {
        self.version.as_ref().and_then(|v| v.as_str())
    }

    pub fn description_str(&self) -> Option<&str> {
        self.description.as_ref().and_then(|v| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_plugin_json() {
        let json = r#"{
            "name": "my-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "keywords": ["test", "dev"]
        }"#;
        let pm: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(pm.name_str(), Some("my-plugin"));
        assert_eq!(pm.version_str(), Some("1.0.0"));
        assert_eq!(pm.description_str(), Some("A test plugin"));
        assert_eq!(pm.keywords, vec!["test", "dev"]);
    }

    #[test]
    fn null_name_returns_none() {
        let json = r#"{"name": null, "version": "1.0.0", "description": "desc"}"#;
        let pm: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(pm.name_str(), None);
    }

    #[test]
    fn missing_fields_are_none() {
        let json = r#"{}"#;
        let pm: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(pm.name_str(), None);
        assert_eq!(pm.version_str(), None);
        assert_eq!(pm.description_str(), None);
    }
}
