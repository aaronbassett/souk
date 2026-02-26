use std::path::PathBuf;

use souk_core::discovery::{discover_marketplace, load_marketplace_config, MarketplaceConfig};
use souk_core::resolution::resolve_plugin;
use souk_core::review::{detect_provider, review_plugin};

use crate::output::Reporter;

/// Run the `souk review plugin` command.
///
/// Resolves the plugin, detects an LLM provider, sends the review prompt,
/// and optionally saves the report to `output_dir`.
pub fn run_review_plugin(
    plugin_input: &str,
    output_dir: Option<&str>,
    provider_name: Option<&str>,
    model: Option<&str>,
    marketplace_override: Option<&str>,
    reporter: &mut Reporter,
) -> bool {
    // Resolve plugin path
    let config = load_config(marketplace_override);
    let plugin_path = match resolve_plugin(plugin_input, config.as_ref()) {
        Ok(p) => p,
        Err(e) => {
            reporter.error(&format!("Failed to resolve plugin: {e}"));
            return false;
        }
    };

    // Detect LLM provider
    let provider = match detect_provider(provider_name, model) {
        Ok(p) => p,
        Err(e) => {
            reporter.error(&format!("{e}"));
            return false;
        }
    };

    reporter.info(&format!(
        "Reviewing plugin with {} ({})",
        provider.name(),
        provider.model()
    ));

    let output_path = output_dir.map(PathBuf::from);
    match review_plugin(&plugin_path, provider.as_ref(), output_path.as_deref()) {
        Ok(report) => {
            reporter.success(&format!("Plugin review complete: {}", report.plugin_name));
            if output_path.is_some() {
                reporter.info("Review report saved");
            }
            true
        }
        Err(e) => {
            reporter.error(&format!("Review failed: {e}"));
            false
        }
    }
}

/// Try to load marketplace config (non-fatal if not found).
fn load_config(marketplace_override: Option<&str>) -> Option<MarketplaceConfig> {
    let mp_path = if let Some(path) = marketplace_override {
        PathBuf::from(path)
    } else {
        let cwd = std::env::current_dir().ok()?;
        discover_marketplace(&cwd).ok()?
    };
    load_marketplace_config(&mp_path).ok()
}
