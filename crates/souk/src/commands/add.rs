//! Handler for the `souk add` CLI command.

use crate::cli::ConflictStrategy;
use crate::output::Reporter;
use souk_core::discovery::MarketplaceConfig;
use souk_core::ops::add::{execute_add, plan_add, ConflictResolution};

/// Run the add command, adding plugins to the marketplace.
///
/// Returns `true` on success, `false` on failure.
pub fn run_add(
    plugins: &[String],
    on_conflict: &ConflictStrategy,
    dry_run: bool,
    no_copy: bool,
    config: &MarketplaceConfig,
    reporter: &mut Reporter,
) -> bool {
    if plugins.is_empty() {
        reporter.error("At least one plugin argument is required");
        return false;
    }

    let strategy = match on_conflict {
        ConflictStrategy::Abort => "abort",
        ConflictStrategy::Skip => "skip",
        ConflictStrategy::Replace => "replace",
        ConflictStrategy::Rename => "rename",
    };

    reporter.section("Pre-flight Validation");

    let plan = match plan_add(plugins, config, strategy, no_copy) {
        Ok(p) => p,
        Err(e) => {
            reporter.error(&format!("{e}"));
            return false;
        }
    };

    if plan.actions.is_empty() {
        reporter.warning("No plugins to add");
        return true;
    }

    // Report plan
    reporter.section("Planning Operations");
    for action in &plan.actions {
        let status = match &action.conflict {
            Some(ConflictResolution::Skip) => "SKIP (already exists)".to_string(),
            Some(ConflictResolution::Replace) => "REPLACE existing".to_string(),
            Some(ConflictResolution::Rename(new_name)) => {
                format!("RENAME -> {new_name}")
            }
            None => {
                if action.is_external {
                    if no_copy {
                        "ADD (external, no copy)".to_string()
                    } else {
                        "ADD (will copy to pluginRoot)".to_string()
                    }
                } else {
                    "ADD (internal)".to_string()
                }
            }
        };
        reporter.info(&format!("{}: {status}", action.plugin_name));
    }

    if dry_run {
        reporter.section("Dry Run");
    }

    match execute_add(&plan, config, dry_run) {
        Ok(added) => {
            if dry_run {
                for name in &added {
                    reporter.info(&format!("Would add: {name}"));
                }
                reporter.warning("Dry run mode - no changes made");
            } else if added.is_empty() {
                reporter.info("No plugins added (all skipped)");
            } else {
                reporter.section("Summary");
                for name in &added {
                    reporter.success(&format!("Added: {name}"));
                }
                reporter.success(&format!(
                    "Successfully added {} plugin(s) to marketplace",
                    added.len()
                ));
            }
            true
        }
        Err(e) => {
            reporter.error(&format!("Add failed: {e}"));
            false
        }
    }
}
