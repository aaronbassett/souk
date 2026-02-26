use std::path::PathBuf;

use souk_core::discovery::{discover_marketplace, load_marketplace_config, MarketplaceConfig};

use crate::output::Reporter;

/// Run pre-commit validation.
///
/// Detects plugins with staged changes and validates only those.
/// If marketplace.json is staged, validates marketplace structure too.
pub fn run_pre_commit(marketplace_override: Option<&str>, reporter: &mut Reporter) -> bool {
    let config = match load_config_required(marketplace_override, reporter) {
        Some(c) => c,
        None => return false,
    };

    reporter.section("Pre-commit validation");

    let result = souk_core::ci::run_pre_commit(&config);
    reporter.report_validation(&result);

    if result.has_errors() {
        reporter.error("Pre-commit validation failed");
        false
    } else {
        reporter.success("Pre-commit validation passed");
        true
    }
}

/// Run pre-push validation.
///
/// Performs full marketplace validation including all plugins.
pub fn run_pre_push(marketplace_override: Option<&str>, reporter: &mut Reporter) -> bool {
    let config = match load_config_required(marketplace_override, reporter) {
        Some(c) => c,
        None => return false,
    };

    reporter.section("Pre-push validation");

    let result = souk_core::ci::run_pre_push(&config);
    reporter.report_validation(&result);

    if result.has_errors() {
        reporter.error("Pre-push validation failed. Use 'git push --no-verify' to skip.");
        false
    } else {
        reporter.success("Pre-push validation passed");
        true
    }
}

fn load_config_required(
    marketplace_override: Option<&str>,
    reporter: &mut Reporter,
) -> Option<MarketplaceConfig> {
    let mp_path = if let Some(path) = marketplace_override {
        PathBuf::from(path)
    } else {
        let cwd = match std::env::current_dir() {
            Ok(c) => c,
            Err(e) => {
                reporter.error(&format!("Cannot get current directory: {e}"));
                return None;
            }
        };
        match discover_marketplace(&cwd) {
            Ok(p) => p,
            Err(e) => {
                reporter.error(&format!("{e}"));
                return None;
            }
        }
    };
    match load_marketplace_config(&mp_path) {
        Ok(c) => Some(c),
        Err(e) => {
            reporter.error(&format!("Failed to load marketplace: {e}"));
            None
        }
    }
}
