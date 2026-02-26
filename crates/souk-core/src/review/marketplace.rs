//! Marketplace-level AI-powered review.
//!
//! Reads the marketplace manifest and all plugin manifests, then sends a
//! structured prompt to an [`LlmProvider`] requesting an overall quality
//! assessment. The resulting report can optionally be persisted to disk.

use std::path::Path;

use crate::discovery::MarketplaceConfig;
use crate::error::SoukError;
use crate::review::provider::LlmProvider;

/// The result of an LLM-powered marketplace review.
#[derive(Debug, Clone)]
pub struct MarketplaceReviewReport {
    /// Name of the LLM provider that generated the review (e.g. "anthropic").
    pub provider_name: String,
    /// Model identifier used for the review (e.g. "claude-sonnet-4-20250514").
    pub model_name: String,
    /// The raw review text returned by the LLM.
    pub review_text: String,
}

/// Review the entire marketplace using an LLM provider.
///
/// This function:
/// 1. Reads `marketplace.json` from `config.marketplace_path`.
/// 2. For each plugin entry, attempts to read its `plugin.json` manifest.
/// 3. Builds a structured review prompt combining the marketplace definition
///    and all plugin summaries.
/// 4. Sends the prompt to `provider` and captures the response.
/// 5. If `output_dir` is provided, writes a Markdown report to
///    `<output_dir>/marketplace-review-report.md`.
///
/// # Errors
///
/// Returns [`SoukError::Io`] if the marketplace file cannot be read, or
/// [`SoukError::LlmApiError`] if the provider call fails.
pub fn review_marketplace(
    config: &MarketplaceConfig,
    provider: &dyn LlmProvider,
    output_dir: Option<&Path>,
) -> Result<MarketplaceReviewReport, SoukError> {
    // 1. Read marketplace.json
    let marketplace_json = std::fs::read_to_string(&config.marketplace_path)?;

    // 2. Read each plugin's plugin.json
    let mut plugin_summaries = Vec::new();
    for entry in &config.marketplace.plugins {
        let plugin_path = config.plugin_root_abs.join(&entry.source);
        let plugin_json_path = plugin_path.join(".claude-plugin").join("plugin.json");
        if let Ok(content) = std::fs::read_to_string(&plugin_json_path) {
            plugin_summaries.push(format!(
                "### {} (source: {})\n```json\n{}\n```",
                entry.name, entry.source, content
            ));
        } else {
            plugin_summaries.push(format!(
                "### {} (source: {}) -- plugin.json not readable",
                entry.name, entry.source
            ));
        }
    }

    // 3. Build prompt
    let prompt = build_marketplace_review_prompt(&marketplace_json, &plugin_summaries);

    // 4. Send to LLM
    let review_text = provider.complete(&prompt)?;

    let report = MarketplaceReviewReport {
        provider_name: provider.name().to_string(),
        model_name: provider.model().to_string(),
        review_text: review_text.clone(),
    };

    // 5. Save report if output_dir is given
    if let Some(dir) = output_dir {
        std::fs::create_dir_all(dir)?;
        let report_path = dir.join("marketplace-review-report.md");
        let content = format!(
            "# Marketplace Review\n\n**Provider:** {} ({})\n\n---\n\n{}\n",
            report.provider_name, report.model_name, review_text,
        );
        std::fs::write(&report_path, content)?;
    }

    Ok(report)
}

/// Build the structured review prompt sent to the LLM.
fn build_marketplace_review_prompt(marketplace_json: &str, plugin_summaries: &[String]) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "You are a senior code reviewer. Review this Claude Code plugin marketplace \
         for quality, consistency, and best practices.\n\n",
    );
    prompt.push_str("## marketplace.json\n```json\n");
    prompt.push_str(marketplace_json);
    prompt.push_str("\n```\n\n");

    if !plugin_summaries.is_empty() {
        prompt.push_str("## Plugins\n\n");
        for summary in plugin_summaries {
            prompt.push_str(summary);
            prompt.push_str("\n\n");
        }
    }

    prompt.push_str("Please provide:\n");
    prompt.push_str("1. Overall marketplace quality assessment\n");
    prompt.push_str("2. Plugin consistency analysis\n");
    prompt.push_str("3. Dependency concerns\n");
    prompt.push_str("4. Naming and organization feedback\n");
    prompt.push_str("5. Suggestions for improvement\n");
    prompt.push_str("6. Overall rating (1-10)\n");

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::provider::MockProvider;
    use crate::types::marketplace::{Marketplace, PluginEntry};
    use tempfile::TempDir;

    /// Helper: create a realistic marketplace on disk and return a `MarketplaceConfig`.
    fn setup_marketplace_config(
        tmp: &TempDir,
        plugins: &[(&str, Option<&str>)],
    ) -> MarketplaceConfig {
        let plugins_dir = tmp.path().join("plugins");
        std::fs::create_dir_all(&plugins_dir).unwrap();

        let mut entries = Vec::new();
        for (name, manifest) in plugins {
            let plugin_dir = plugins_dir.join(name);
            let claude_dir = plugin_dir.join(".claude-plugin");
            std::fs::create_dir_all(&claude_dir).unwrap();

            if let Some(content) = manifest {
                std::fs::write(claude_dir.join("plugin.json"), content).unwrap();
            }

            entries.push(PluginEntry {
                name: name.to_string(),
                source: name.to_string(),
                tags: vec![],
            });
        }

        let marketplace = Marketplace {
            version: "0.1.0".to_string(),
            plugin_root: Some("./plugins".to_string()),
            plugins: entries,
        };

        let claude_dir = tmp.path().join(".claude-plugin");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let marketplace_path = claude_dir.join("marketplace.json");
        let marketplace_json = serde_json::to_string_pretty(&marketplace).unwrap();
        std::fs::write(&marketplace_path, &marketplace_json).unwrap();

        MarketplaceConfig {
            marketplace_path,
            project_root: tmp.path().to_path_buf(),
            plugin_root_abs: plugins_dir,
            marketplace,
        }
    }

    #[test]
    fn review_marketplace_builds_prompt_from_plugins() {
        let tmp = TempDir::new().unwrap();
        let plugin_manifest = r#"{"name": "greeter", "description": "Says hello"}"#;
        let config = setup_marketplace_config(&tmp, &[("greeter", Some(plugin_manifest))]);

        let provider = MockProvider::new("Looks great! Rating: 9/10");
        let report = review_marketplace(&config, &provider, None).unwrap();

        assert_eq!(report.provider_name, "mock");
        assert_eq!(report.model_name, "mock-model");
        assert_eq!(report.review_text, "Looks great! Rating: 9/10");
    }

    #[test]
    fn review_marketplace_saves_report_to_output_dir() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_config(&tmp, &[("alpha", Some(r#"{"name": "alpha"}"#))]);

        let output_dir = tmp.path().join("reviews");
        let provider = MockProvider::new("Overall: solid marketplace.");
        let report = review_marketplace(&config, &provider, Some(&output_dir)).unwrap();

        let report_path = output_dir.join("marketplace-review-report.md");
        assert!(
            report_path.exists(),
            "Report file should be written to disk"
        );

        let content = std::fs::read_to_string(&report_path).unwrap();
        assert!(content.contains("# Marketplace Review"));
        assert!(content.contains("mock"));
        assert!(content.contains("mock-model"));
        assert!(content.contains(&report.review_text));
    }

    #[test]
    fn review_marketplace_works_with_empty_marketplace() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace_config(&tmp, &[]);

        let provider = MockProvider::new("Empty marketplace, structure looks fine.");
        let report = review_marketplace(&config, &provider, None).unwrap();

        assert_eq!(
            report.review_text,
            "Empty marketplace, structure looks fine."
        );
        assert_eq!(report.provider_name, "mock");
    }

    #[test]
    fn review_marketplace_handles_unreadable_plugin_manifest() {
        let tmp = TempDir::new().unwrap();
        // Create one plugin with a manifest and one without (no plugin.json file).
        let config = setup_marketplace_config(
            &tmp,
            &[
                ("good-plugin", Some(r#"{"name": "good-plugin"}"#)),
                ("bad-plugin", None),
            ],
        );

        let provider = MockProvider::new("Mixed quality.");
        let report = review_marketplace(&config, &provider, None).unwrap();

        // The function should still succeed even if a plugin.json is missing.
        assert_eq!(report.review_text, "Mixed quality.");
    }

    #[test]
    fn build_prompt_contains_marketplace_json() {
        let marketplace_json = r#"{"version": "0.1.0", "plugins": []}"#;
        let prompt = build_marketplace_review_prompt(marketplace_json, &[]);

        assert!(prompt.contains(marketplace_json));
        assert!(prompt.contains("senior code reviewer"));
        assert!(prompt.contains("Overall rating (1-10)"));
    }

    #[test]
    fn build_prompt_includes_plugin_summaries() {
        let marketplace_json = r#"{"version": "0.1.0"}"#;
        let summaries = vec![
            "### alpha (source: alpha)\n```json\n{}\n```".to_string(),
            "### beta (source: beta) -- plugin.json not readable".to_string(),
        ];

        let prompt = build_marketplace_review_prompt(marketplace_json, &summaries);

        assert!(prompt.contains("## Plugins"));
        assert!(prompt.contains("### alpha"));
        assert!(prompt.contains("### beta"));
        assert!(prompt.contains("plugin.json not readable"));
    }
}
