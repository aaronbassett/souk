//! Handler for the `souk remove` CLI command.

use crate::output::Reporter;
use souk_core::discovery::MarketplaceConfig;
use souk_core::ops::remove::remove_plugins;

/// Run the remove command, removing plugins from the marketplace.
///
/// Returns `true` on success, `false` on failure.
pub fn run_remove(
    plugins: &[String],
    delete: bool,
    allow_external_delete: bool,
    config: &MarketplaceConfig,
    reporter: &mut Reporter,
) -> bool {
    if plugins.is_empty() {
        reporter.error("At least one plugin name is required");
        return false;
    }

    reporter.section("Removing Plugins");

    match remove_plugins(plugins, delete, allow_external_delete, config) {
        Ok(removed) => {
            if removed.is_empty() {
                reporter.info("No plugins removed");
            } else {
                reporter.section("Summary");
                for name in &removed {
                    if delete {
                        reporter.success(&format!("Removed and deleted: {name}"));
                    } else {
                        reporter.success(&format!("Removed: {name}"));
                    }
                }
                reporter.success(&format!(
                    "Successfully removed {} plugin(s) from marketplace",
                    removed.len()
                ));
            }
            true
        }
        Err(e) => {
            reporter.error(&format!("Remove failed: {e}"));
            false
        }
    }
}
