use std::path::PathBuf;
use thiserror::Error;

/// Severity of a validation diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single validation finding.
#[derive(Debug, Clone)]
pub struct ValidationDiagnostic {
    pub severity: Severity,
    pub message: String,
    pub path: Option<PathBuf>,
    pub field: Option<String>,
}

impl ValidationDiagnostic {
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            path: None,
            field: None,
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            path: None,
            field: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// The result of validating a plugin or marketplace.
#[derive(Debug)]
pub struct ValidationResult {
    pub diagnostics: Vec<ValidationDiagnostic>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    pub fn push(&mut self, diagnostic: ValidationDiagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    pub fn warning_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }

    pub fn merge(&mut self, other: ValidationResult) {
        self.diagnostics.extend(other.diagnostics);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum SoukError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Skill not found: {skill} in plugin {plugin}")]
    SkillNotFound { plugin: String, skill: String },

    #[error("Marketplace not found: searched upward from {0}")]
    MarketplaceNotFound(PathBuf),

    #[error("Marketplace already exists at {0}")]
    MarketplaceAlreadyExists(PathBuf),

    #[error("Plugin already exists in marketplace: {0}")]
    PluginAlreadyExists(String),

    #[error("Validation failed with {0} error(s)")]
    ValidationFailed(usize),

    #[error("Atomic operation failed, backup restored: {0}")]
    AtomicRollback(String),

    #[error("No LLM API key found. Set one of: ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY")]
    NoApiKey,

    #[error("LLM API error: {0}")]
    LlmApiError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Semver error: {0}")]
    Semver(#[from] semver::Error),

    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_result_tracks_errors_and_warnings() {
        let mut result = ValidationResult::new();
        result.push(ValidationDiagnostic::error("bad thing"));
        result.push(ValidationDiagnostic::warning("meh thing"));
        result.push(ValidationDiagnostic::error("another bad"));

        assert!(result.has_errors());
        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);
    }

    #[test]
    fn validation_result_merge() {
        let mut a = ValidationResult::new();
        a.push(ValidationDiagnostic::error("a"));

        let mut b = ValidationResult::new();
        b.push(ValidationDiagnostic::warning("b"));

        a.merge(b);
        assert_eq!(a.diagnostics.len(), 2);
    }

    #[test]
    fn diagnostic_builder_pattern() {
        let d = ValidationDiagnostic::error("missing name")
            .with_path("/tmp/plugin")
            .with_field("name");

        assert!(d.is_error());
        assert_eq!(d.path.unwrap().to_str().unwrap(), "/tmp/plugin");
        assert_eq!(d.field.unwrap(), "name");
    }
}
