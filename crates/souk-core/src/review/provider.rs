//! LLM provider abstraction for AI-powered reviews.
//!
//! Supports Anthropic, OpenAI, and Gemini APIs with automatic provider
//! detection from environment variables. See decision D4 in the project
//! spec: all LLM interaction goes through direct API calls, not CLI tools.

use crate::error::SoukError;

/// Trait for LLM API providers.
///
/// Implementations must be `Send + Sync` so providers can be shared across
/// threads or stored in async contexts. The `complete` method is synchronous
/// (using `reqwest::blocking`) because review operations are inherently
/// sequential and the added complexity of async is not justified here.
pub trait LlmProvider: Send + Sync {
    /// Send a prompt and return the completion text.
    fn complete(&self, prompt: &str) -> Result<String, SoukError>;

    /// Provider name (e.g., "anthropic", "openai", "gemini").
    fn name(&self) -> &str;

    /// Model identifier being used (e.g., "claude-sonnet-4-20250514").
    fn model(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Anthropic
// ---------------------------------------------------------------------------

/// LLM provider for the Anthropic Messages API.
pub struct AnthropicProvider {
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    ///
    /// If `model` is `None`, defaults to `claude-sonnet-4-20250514`.
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl LlmProvider for AnthropicProvider {
    fn complete(&self, prompt: &str) -> Result<String, SoukError> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {"role": "user", "content": prompt}
            ]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| SoukError::LlmApiError(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(SoukError::LlmApiError(format!("HTTP {status}: {text}")));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| SoukError::LlmApiError(format!("Failed to parse response: {e}")))?;

        json["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SoukError::LlmApiError("No text in response".into()))
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// OpenAI
// ---------------------------------------------------------------------------

/// LLM provider for the OpenAI Chat Completions API.
pub struct OpenAiProvider {
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider.
    ///
    /// If `model` is `None`, defaults to `gpt-4o`.
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "gpt-4o".to_string()),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl LlmProvider for OpenAiProvider {
    fn complete(&self, prompt: &str) -> Result<String, SoukError> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "max_tokens": 4096
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| SoukError::LlmApiError(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(SoukError::LlmApiError(format!("HTTP {status}: {text}")));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| SoukError::LlmApiError(format!("Failed to parse response: {e}")))?;

        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SoukError::LlmApiError("No content in response".into()))
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// Gemini
// ---------------------------------------------------------------------------

/// LLM provider for the Google Gemini generateContent API.
pub struct GeminiProvider {
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl GeminiProvider {
    /// Create a new Gemini provider.
    ///
    /// If `model` is `None`, defaults to `gemini-2.0-flash`.
    pub fn new(api_key: String, model: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "gemini-2.0-flash".to_string()),
            client: reqwest::blocking::Client::new(),
        }
    }
}

impl LlmProvider for GeminiProvider {
    fn complete(&self, prompt: &str) -> Result<String, SoukError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );

        let body = serde_json::json!({
            "contents": [
                {"parts": [{"text": prompt}]}
            ]
        });

        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| SoukError::LlmApiError(format!("Request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(SoukError::LlmApiError(format!("HTTP {status}: {text}")));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| SoukError::LlmApiError(format!("Failed to parse response: {e}")))?;

        json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| SoukError::LlmApiError("No text in response".into()))
    }

    fn name(&self) -> &str {
        "gemini"
    }

    fn model(&self) -> &str {
        &self.model
    }
}

// ---------------------------------------------------------------------------
// Mock (for testing)
// ---------------------------------------------------------------------------

/// A mock LLM provider that returns a fixed response. For use in tests.
pub struct MockProvider {
    response: String,
}

impl MockProvider {
    /// Create a mock provider that always returns the given response.
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

impl LlmProvider for MockProvider {
    fn complete(&self, _prompt: &str) -> Result<String, SoukError> {
        Ok(self.response.clone())
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }
}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// Detect the best available LLM provider from environment variables.
///
/// Priority order: `ANTHROPIC_API_KEY` > `OPENAI_API_KEY` > `GEMINI_API_KEY`.
///
/// Use `provider_override` (from `--provider` flag) to force a specific
/// provider. Use `model_override` (from `--model` flag) to override the
/// default model for the selected provider.
///
/// Returns `SoukError::NoApiKey` if no provider can be configured, or
/// `SoukError::Other` if an unknown provider name is given.
pub fn detect_provider(
    provider_override: Option<&str>,
    model_override: Option<&str>,
) -> Result<Box<dyn LlmProvider>, SoukError> {
    let model = model_override.map(|s| s.to_string());

    if let Some(provider_name) = provider_override {
        return match provider_name {
            "anthropic" => {
                let key =
                    std::env::var("ANTHROPIC_API_KEY").map_err(|_| SoukError::NoApiKey)?;
                Ok(Box::new(AnthropicProvider::new(key, model)))
            }
            "openai" => {
                let key = std::env::var("OPENAI_API_KEY").map_err(|_| SoukError::NoApiKey)?;
                Ok(Box::new(OpenAiProvider::new(key, model)))
            }
            "gemini" => {
                let key = std::env::var("GEMINI_API_KEY").map_err(|_| SoukError::NoApiKey)?;
                Ok(Box::new(GeminiProvider::new(key, model)))
            }
            _ => Err(SoukError::Other(format!(
                "Unknown provider: {provider_name}"
            ))),
        };
    }

    // Auto-detect: try providers in priority order.
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        return Ok(Box::new(AnthropicProvider::new(key, model)));
    }
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        return Ok(Box::new(OpenAiProvider::new(key, model)));
    }
    if let Ok(key) = std::env::var("GEMINI_API_KEY") {
        return Ok(Box::new(GeminiProvider::new(key, model)));
    }

    Err(SoukError::NoApiKey)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_returns_expected_response() {
        let provider = MockProvider::new("This is a review.");
        let result = provider.complete("Review this plugin").unwrap();
        assert_eq!(result, "This is a review.");
    }

    #[test]
    fn mock_provider_name_and_model() {
        let provider = MockProvider::new("ignored");
        assert_eq!(provider.name(), "mock");
        assert_eq!(provider.model(), "mock-model");
    }

    #[test]
    fn detect_provider_no_env_vars_returns_no_api_key() {
        // Clear all relevant env vars so auto-detection fails.
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("GEMINI_API_KEY");

        let result = detect_provider(None, None);
        match result {
            Err(SoukError::NoApiKey) => {} // expected
            Err(other) => panic!("Expected NoApiKey, got: {other:?}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn detect_provider_with_anthropic_key() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        // Ensure others are not set so we hit Anthropic first.
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("GEMINI_API_KEY");

        let provider = detect_provider(None, None).unwrap();
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.model(), "claude-sonnet-4-20250514");

        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn detect_provider_with_openai_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::set_var("OPENAI_API_KEY", "test-key-456");
        std::env::remove_var("GEMINI_API_KEY");

        let provider = detect_provider(None, None).unwrap();
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model(), "gpt-4o");

        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn detect_provider_with_gemini_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::set_var("GEMINI_API_KEY", "test-key-789");

        let provider = detect_provider(None, None).unwrap();
        assert_eq!(provider.name(), "gemini");
        assert_eq!(provider.model(), "gemini-2.0-flash");

        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn detect_provider_priority_anthropic_over_openai() {
        std::env::set_var("ANTHROPIC_API_KEY", "key-a");
        std::env::set_var("OPENAI_API_KEY", "key-o");
        std::env::remove_var("GEMINI_API_KEY");

        let provider = detect_provider(None, None).unwrap();
        assert_eq!(provider.name(), "anthropic");

        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn detect_provider_explicit_override() {
        std::env::set_var("OPENAI_API_KEY", "key-o");
        std::env::remove_var("ANTHROPIC_API_KEY");
        std::env::remove_var("GEMINI_API_KEY");

        let provider = detect_provider(Some("openai"), None).unwrap();
        assert_eq!(provider.name(), "openai");

        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn detect_provider_explicit_override_missing_key() {
        std::env::remove_var("ANTHROPIC_API_KEY");

        let result = detect_provider(Some("anthropic"), None);
        match result {
            Err(SoukError::NoApiKey) => {} // expected
            Err(other) => panic!("Expected NoApiKey, got: {other:?}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn detect_provider_unknown_provider() {
        let result = detect_provider(Some("unknown-provider"), None);
        match result {
            Err(SoukError::Other(msg)) => {
                assert!(msg.contains("Unknown provider"), "Unexpected message: {msg}");
            }
            Err(other) => panic!("Expected Other, got: {other:?}"),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn detect_provider_model_override() {
        std::env::set_var("ANTHROPIC_API_KEY", "key-a");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("GEMINI_API_KEY");

        let provider = detect_provider(None, Some("claude-opus-4-20250514")).unwrap();
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.model(), "claude-opus-4-20250514");

        std::env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn provider_trait_is_object_safe() {
        // Verify LlmProvider can be used as a trait object.
        let provider: Box<dyn LlmProvider> = Box::new(MockProvider::new("test"));
        assert_eq!(provider.name(), "mock");
        assert_eq!(provider.complete("anything").unwrap(), "test");
    }

    #[test]
    fn anthropic_provider_default_model() {
        let provider = AnthropicProvider::new("key".into(), None);
        assert_eq!(provider.model(), "claude-sonnet-4-20250514");
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn openai_provider_default_model() {
        let provider = OpenAiProvider::new("key".into(), None);
        assert_eq!(provider.model(), "gpt-4o");
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn gemini_provider_default_model() {
        let provider = GeminiProvider::new("key".into(), None);
        assert_eq!(provider.model(), "gemini-2.0-flash");
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn provider_custom_model() {
        let provider = AnthropicProvider::new("key".into(), Some("custom-model".into()));
        assert_eq!(provider.model(), "custom-model");
    }
}
