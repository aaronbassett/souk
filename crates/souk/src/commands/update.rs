//! Handler for the `souk update` CLI command.

use crate::output::Reporter;
use souk_core::discovery::MarketplaceConfig;
use souk_core::ops::update::update_plugins;

/// Run the update command, refreshing plugin metadata and optionally bumping versions.
///
/// Returns `true` on success, `false` on failure.
pub fn run_update(
    plugins: &[String],
    bump_type: Option<&str>,
    config: &MarketplaceConfig,
    reporter: &mut Reporter,
) -> bool {
    if plugins.is_empty() {
        reporter.error("At least one plugin name is required");
        return false;
    }

    reporter.section("Updating Plugins");

    if let Some(bump) = bump_type {
        reporter.info(&format!("Version bump: {bump}"));
    }

    match update_plugins(plugins, bump_type, config) {
        Ok(updated) => {
            if updated.is_empty() {
                reporter.info("No plugins updated");
            } else {
                reporter.section("Summary");
                for name in &updated {
                    reporter.success(&format!("Updated: {name}"));
                }
                reporter.success(&format!("Successfully updated {} plugin(s)", updated.len()));
            }
            true
        }
        Err(e) => {
            reporter.error(&format!("Update failed: {e}"));
            false
        }
    }
}
