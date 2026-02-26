//! CI hook execution logic.
//!
//! This module contains the logic for running `souk ci run pre-commit`
//! and `souk ci run pre-push` commands. Hook execution invokes the
//! appropriate validation based on the hook type.

use std::path::Path;

use crate::discovery::{discover_marketplace, load_marketplace_config};
use crate::error::{SoukError, ValidationResult};
use crate::validation;

/// Run pre-commit validation on the given project root.
///
/// Discovers the marketplace, then validates changed plugins.
/// Returns the overall validation result.
pub fn run_pre_commit(project_root: &Path) -> Result<ValidationResult, SoukError> {
    // For pre-commit, validate individual plugins at the project root level.
    // If a marketplace exists, validate all plugins; otherwise validate
    // the directory as a single plugin.
    let mp_path = discover_marketplace(project_root);

    match mp_path {
        Ok(path) => {
            let config = load_marketplace_config(&path)?;
            Ok(validation::marketplace::validate_marketplace(&config, false))
        }
        Err(_) => {
            // No marketplace; try validating as a single plugin directory
            Ok(validation::plugin::validate_plugin(project_root))
        }
    }
}

/// Run pre-push validation on the given project root.
///
/// Discovers the marketplace and runs comprehensive validation.
/// Returns the overall validation result.
pub fn run_pre_push(project_root: &Path) -> Result<ValidationResult, SoukError> {
    let mp_path = discover_marketplace(project_root)?;
    let config = load_marketplace_config(&mp_path)?;
    Ok(validation::marketplace::validate_marketplace(&config, false))
}
