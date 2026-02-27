//! Handler for the `souk prune` CLI command.

use crate::output::Reporter;
use souk_core::discovery::MarketplaceConfig;
use souk_core::ops::prune::prune_plugins;

/// Run the prune command, removing orphaned plugin directories.
///
/// Returns `true` on success, `false` on failure.
pub fn run_prune(apply: bool, config: &MarketplaceConfig, reporter: &mut Reporter) -> bool {
    match prune_plugins(apply, config) {
        Ok(result) => {
            if result.orphaned.is_empty() {
                reporter.info("No orphaned plugin directories found.");
                return true;
            }

            if apply {
                reporter.section("Prune");
                for path in &result.deleted {
                    let name = path.file_name().unwrap().to_string_lossy();
                    reporter.success(&format!("Deleted: {name}"));
                }
                for warn in &result.warnings {
                    reporter.warning(warn);
                }
                reporter.success(&format!(
                    "Successfully pruned {} orphaned plugin directory(ies).",
                    result.deleted.len()
                ));
            } else {
                reporter.section("Prune (dry-run)");
                for path in &result.orphaned {
                    let name = path.file_name().unwrap().to_string_lossy();
                    reporter.info(&format!("Would delete: {name}"));
                }
                reporter.info(&format!(
                    "Found {} orphaned plugin directory(ies). Run with --apply to delete.",
                    result.orphaned.len()
                ));
            }

            true
        }
        Err(e) => {
            reporter.error(&format!("Prune failed: {e}"));
            false
        }
    }
}
