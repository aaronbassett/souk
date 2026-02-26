//! Handler for the `souk init` CLI command.

use std::path::Path;

use souk_core::ops::init::scaffold_marketplace;

use crate::output::Reporter;

/// Run the init command, scaffolding a new marketplace at `target_path`.
///
/// Returns `true` on success, `false` on failure.
pub fn run_init(target_path: &str, plugin_root: &str, reporter: &mut Reporter) -> bool {
    let path = Path::new(target_path);

    match scaffold_marketplace(path, plugin_root) {
        Ok(()) => {
            reporter.success(&format!("Marketplace initialized at {}", path.display()));
            reporter.info(&format!(
                "Created .claude-plugin/marketplace.json with pluginRoot: {plugin_root}"
            ));
            true
        }
        Err(souk_core::SoukError::MarketplaceAlreadyExists(mp_path)) => {
            reporter.error(&format!(
                "Marketplace already exists at {}",
                mp_path.display()
            ));
            false
        }
        Err(e) => {
            reporter.error(&format!("Failed to initialize marketplace: {e}"));
            false
        }
    }
}
