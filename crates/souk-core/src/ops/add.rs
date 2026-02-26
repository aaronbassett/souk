//! Add plugins to the marketplace.
//!
//! Implements the 7-phase pipeline for adding plugins:
//! 1. Preflight: Resolve each plugin path, validate it
//! 2. Plan: Determine if internal or external, check for conflicts
//! 3. Dry-run gate: If dry run, report planned actions and stop
//! 4. Copy: For external plugins, copy to pluginRoot
//! 5. Atomic update: Use AtomicGuard, add entries, write back
//! 6. Version bump: Bump marketplace version (patch)
//! 7. Final validation: Re-validate the marketplace

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::discovery::{load_marketplace_config, MarketplaceConfig};
use crate::error::SoukError;
use crate::ops::AtomicGuard;
use crate::resolution::{plugin_path_to_source, resolve_plugin};
use crate::types::{Marketplace, PluginEntry, PluginManifest};
use crate::validation::{validate_marketplace, validate_plugin};
use crate::version::{bump_patch, generate_unique_name};

/// A planned action for adding a single plugin.
#[derive(Debug, Clone)]
pub struct AddAction {
    /// Resolved path to the plugin directory on disk.
    pub plugin_path: PathBuf,
    /// The name of the plugin (from plugin.json).
    pub plugin_name: String,
    /// The source value for the marketplace entry.
    pub source: String,
    /// Whether the plugin is already under pluginRoot.
    pub is_external: bool,
    /// How to resolve a name conflict, if one exists.
    pub conflict: Option<ConflictResolution>,
}

/// How a name conflict should be resolved for a single plugin.
#[derive(Debug, Clone)]
pub enum ConflictResolution {
    /// Skip this plugin entirely.
    Skip,
    /// Replace the existing entry with the new one.
    Replace,
    /// Rename the new plugin to avoid conflict.
    Rename(String),
}

/// The full plan produced by the planning phase.
#[derive(Debug, Clone)]
pub struct AddPlan {
    pub actions: Vec<AddAction>,
}

/// Plans the add operation without modifying the filesystem.
///
/// Resolves each input to a plugin path, reads its plugin.json, determines
/// internal vs external, and applies the conflict resolution strategy.
///
/// # Arguments
///
/// * `inputs` - Plugin paths or names to add.
/// * `config` - The loaded marketplace configuration.
/// * `strategy` - One of "abort", "skip", "replace", or "rename".
/// * `no_copy` - If true, external plugins will be referenced by absolute path
///   instead of being copied into pluginRoot.
///
/// # Errors
///
/// Returns [`SoukError::PluginNotFound`] if a plugin cannot be resolved.
/// Returns [`SoukError::PluginAlreadyExists`] if the strategy is "abort" and a
/// conflict is detected.
/// Returns [`SoukError::ValidationFailed`] if preflight validation fails.
pub fn plan_add(
    inputs: &[String],
    config: &MarketplaceConfig,
    strategy: &str,
    no_copy: bool,
) -> Result<AddPlan, SoukError> {
    let existing_names: HashSet<String> = config
        .marketplace
        .plugins
        .iter()
        .map(|p| p.name.clone())
        .collect();

    let mut actions = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for input in inputs {
        // Phase 1: Resolve plugin path
        let plugin_path = match resolve_plugin_input(input, config) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("Plugin not found: {input} ({e})"));
                continue;
            }
        };

        // Read plugin.json to get the name
        let manifest = read_plugin_manifest(&plugin_path)?;
        let plugin_name = manifest
            .name_str()
            .ok_or_else(|| {
                SoukError::Other(format!(
                    "Plugin has no name in plugin.json: {}",
                    plugin_path.display()
                ))
            })?
            .to_string();

        // Validate the plugin
        let validation = validate_plugin(&plugin_path);
        if validation.has_errors() {
            errors.push(format!(
                "Plugin validation failed: {plugin_name} ({})",
                plugin_path.display()
            ));
            continue;
        }

        // Phase 2: Determine internal vs external
        let (source, is_internal) = plugin_path_to_source(&plugin_path, config);
        let is_external = !is_internal;

        // Determine the final source for the marketplace entry
        let final_source = if is_external && !no_copy {
            // Will be copied to pluginRoot; source = the plugin name (directory name)
            plugin_name.clone()
        } else {
            source
        };

        // Check for conflicts
        let conflict = if existing_names.contains(&plugin_name) {
            match strategy {
                "abort" => {
                    return Err(SoukError::PluginAlreadyExists(plugin_name));
                }
                "skip" => Some(ConflictResolution::Skip),
                "replace" => Some(ConflictResolution::Replace),
                "rename" => {
                    let new_name = generate_unique_name(&plugin_name, &existing_names);
                    Some(ConflictResolution::Rename(new_name))
                }
                _ => {
                    return Err(SoukError::Other(format!(
                        "Invalid conflict strategy: {strategy}"
                    )));
                }
            }
        } else {
            None
        };

        actions.push(AddAction {
            plugin_path,
            plugin_name,
            source: final_source,
            is_external,
            conflict,
        });
    }

    if !errors.is_empty() {
        return Err(SoukError::Other(errors.join("; ")));
    }

    Ok(AddPlan { actions })
}

/// Executes the add plan, modifying the filesystem and marketplace.json.
///
/// If `dry_run` is true, no changes are made and the function returns early
/// after the planning phase.
///
/// # Errors
///
/// Returns an error if copying, atomic update, version bump, or final
/// validation fails. On atomic update failure, the AtomicGuard restores
/// the original marketplace.json.
pub fn execute_add(
    plan: &AddPlan,
    config: &MarketplaceConfig,
    dry_run: bool,
) -> Result<Vec<String>, SoukError> {
    // Collect the effective actions (skip those marked Skip)
    let effective_actions: Vec<&AddAction> = plan
        .actions
        .iter()
        .filter(|a| !matches!(a.conflict, Some(ConflictResolution::Skip)))
        .collect();

    if effective_actions.is_empty() {
        return Ok(Vec::new());
    }

    // Phase 3: Dry-run gate
    if dry_run {
        let names: Vec<String> = effective_actions
            .iter()
            .map(|a| match &a.conflict {
                Some(ConflictResolution::Rename(new_name)) => new_name.clone(),
                _ => a.plugin_name.clone(),
            })
            .collect();
        return Ok(names);
    }

    // Phase 4: Copy external plugins
    for action in &effective_actions {
        if action.is_external && !action.source.starts_with('/') {
            let target_name = match &action.conflict {
                Some(ConflictResolution::Rename(new_name)) => new_name.as_str(),
                _ => &action.source,
            };
            let target_dir = config.plugin_root_abs.join(target_name);

            if target_dir.exists() && !matches!(action.conflict, Some(ConflictResolution::Replace))
            {
                return Err(SoukError::Other(format!(
                    "Target directory already exists: {}",
                    target_dir.display()
                )));
            }

            if matches!(action.conflict, Some(ConflictResolution::Replace)) && target_dir.exists()
            {
                fs::remove_dir_all(&target_dir)?;
            }

            copy_dir_recursive(&action.plugin_path, &target_dir)?;
        }
    }

    // Phase 5: Atomic update
    let guard = AtomicGuard::new(&config.marketplace_path)?;

    let content = fs::read_to_string(&config.marketplace_path)?;
    let mut marketplace: Marketplace = serde_json::from_str(&content)?;

    let mut added_names = Vec::new();

    for action in &effective_actions {
        let (final_name, final_source) = match &action.conflict {
            Some(ConflictResolution::Replace) => {
                // Remove existing entry
                marketplace
                    .plugins
                    .retain(|p| p.name != action.plugin_name);
                (action.plugin_name.clone(), action.source.clone())
            }
            Some(ConflictResolution::Rename(new_name)) => {
                (new_name.clone(), new_name.clone())
            }
            Some(ConflictResolution::Skip) => continue,
            None => (action.plugin_name.clone(), action.source.clone()),
        };

        // Read tags from plugin.json
        let manifest = read_plugin_manifest(&action.plugin_path)?;
        let tags = manifest.keywords;

        marketplace.plugins.push(PluginEntry {
            name: final_name.clone(),
            source: final_source,
            tags,
        });

        added_names.push(final_name);
    }

    // Phase 6: Version bump (patch)
    marketplace.version = bump_patch(&marketplace.version)?;

    // Write back
    let json = serde_json::to_string_pretty(&marketplace)?;
    fs::write(&config.marketplace_path, format!("{json}\n"))?;

    // Phase 7: Final validation
    let updated_config = load_marketplace_config(&config.marketplace_path)?;
    let validation = validate_marketplace(&updated_config, true);
    if validation.has_errors() {
        // Let the guard drop to restore the backup
        drop(guard);
        return Err(SoukError::AtomicRollback(
            "Final validation failed after add".to_string(),
        ));
    }

    // Commit the guard (removes backup)
    guard.commit()?;

    Ok(added_names)
}

/// Resolves a plugin input (path or name) to an absolute path.
fn resolve_plugin_input(input: &str, config: &MarketplaceConfig) -> Result<PathBuf, SoukError> {
    let input_path = PathBuf::from(input);

    // Try as a direct path first
    if input_path.is_dir() {
        return input_path.canonicalize().map_err(SoukError::Io);
    }

    // Try resolving via plugin resolution
    resolve_plugin(input, Some(config))
}

/// Reads and parses plugin.json from a plugin directory.
fn read_plugin_manifest(plugin_path: &Path) -> Result<PluginManifest, SoukError> {
    let plugin_json = plugin_path
        .join(".claude-plugin")
        .join("plugin.json");

    let content = fs::read_to_string(&plugin_json).map_err(|e| {
        SoukError::Other(format!(
            "Cannot read plugin.json at {}: {e}",
            plugin_json.display()
        ))
    })?;

    let manifest: PluginManifest = serde_json::from_str(&content)?;
    Ok(manifest)
}

/// Recursively copies a directory from `src` to `dst`.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), SoukError> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::load_marketplace_config;
    use tempfile::TempDir;

    /// Creates a minimal marketplace setup in a temp directory.
    fn setup_marketplace(tmp: &TempDir, plugins_json: &str) -> MarketplaceConfig {
        let claude_dir = tmp.path().join(".claude-plugin");
        fs::create_dir_all(&claude_dir).unwrap();
        let plugins_dir = tmp.path().join("plugins");
        fs::create_dir_all(&plugins_dir).unwrap();

        let mp_json = format!(
            r#"{{"version":"0.1.0","pluginRoot":"./plugins","plugins":[{plugins_json}]}}"#
        );
        fs::write(claude_dir.join("marketplace.json"), &mp_json).unwrap();
        load_marketplace_config(&claude_dir.join("marketplace.json")).unwrap()
    }

    /// Creates a valid plugin directory.
    fn create_plugin(base: &Path, name: &str) -> PathBuf {
        let plugin_dir = base.join(name);
        let claude_dir = plugin_dir.join(".claude-plugin");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("plugin.json"),
            format!(
                r#"{{"name":"{name}","version":"1.0.0","description":"A test plugin","keywords":["test"]}}"#
            ),
        )
        .unwrap();
        plugin_dir
    }

    #[test]
    fn add_single_plugin_to_empty_marketplace() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(&tmp, "");

        // Create a plugin inside pluginRoot
        create_plugin(&config.plugin_root_abs, "my-plugin");

        let plan = plan_add(
            &["my-plugin".to_string()],
            &config,
            "abort",
            false,
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].plugin_name, "my-plugin");
        assert!(!plan.actions[0].is_external);
        assert!(plan.actions[0].conflict.is_none());

        let added = execute_add(&plan, &config, false).unwrap();
        assert_eq!(added, vec!["my-plugin"]);

        // Verify marketplace was updated
        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].name, "my-plugin");
        assert_eq!(mp.plugins[0].tags, vec!["test"]);
        // Version should be bumped
        assert_eq!(mp.version, "0.1.1");
    }

    #[test]
    fn add_with_conflict_abort_strategy() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"name":"existing","source":"existing","tags":[]}"#,
        );
        create_plugin(&config.plugin_root_abs, "existing");

        let result = plan_add(
            &["existing".to_string()],
            &config,
            "abort",
            false,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            SoukError::PluginAlreadyExists(name) => assert_eq!(name, "existing"),
            other => panic!("Expected PluginAlreadyExists, got: {other}"),
        }
    }

    #[test]
    fn add_with_skip_strategy() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"name":"existing","source":"existing","tags":[]}"#,
        );
        create_plugin(&config.plugin_root_abs, "existing");

        let plan = plan_add(
            &["existing".to_string()],
            &config,
            "skip",
            false,
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 1);
        assert!(matches!(
            plan.actions[0].conflict,
            Some(ConflictResolution::Skip)
        ));

        // Execute should not add anything
        let added = execute_add(&plan, &config, false).unwrap();
        assert!(added.is_empty());

        // Marketplace should be unchanged
        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.version, "0.1.0");
    }

    #[test]
    fn add_with_replace_strategy() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"name":"existing","source":"existing","tags":["old"]}"#,
        );
        create_plugin(&config.plugin_root_abs, "existing");

        let plan = plan_add(
            &["existing".to_string()],
            &config,
            "replace",
            false,
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 1);
        assert!(matches!(
            plan.actions[0].conflict,
            Some(ConflictResolution::Replace)
        ));

        let added = execute_add(&plan, &config, false).unwrap();
        assert_eq!(added, vec!["existing"]);

        // Tags should be updated from plugin.json
        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].tags, vec!["test"]);
        assert_eq!(mp.version, "0.1.1");
    }

    #[test]
    fn add_with_rename_strategy() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(
            &tmp,
            r#"{"name":"existing","source":"existing","tags":[]}"#,
        );
        create_plugin(&config.plugin_root_abs, "existing");

        let plan = plan_add(
            &["existing".to_string()],
            &config,
            "rename",
            false,
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 1);
        match &plan.actions[0].conflict {
            Some(ConflictResolution::Rename(new_name)) => {
                assert_eq!(new_name, "existing-2");
            }
            other => panic!("Expected Rename, got: {other:?}"),
        }
    }

    #[test]
    fn dry_run_does_not_modify_files() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(&tmp, "");
        create_plugin(&config.plugin_root_abs, "my-plugin");

        let plan = plan_add(
            &["my-plugin".to_string()],
            &config,
            "abort",
            false,
        )
        .unwrap();

        let added = execute_add(&plan, &config, true).unwrap();
        assert_eq!(added, vec!["my-plugin"]);

        // Marketplace should be unchanged
        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert!(mp.plugins.is_empty());
        assert_eq!(mp.version, "0.1.0");
    }

    #[test]
    fn external_plugin_copy() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(&tmp, "");

        // Create plugin outside pluginRoot
        let external_dir = TempDir::new().unwrap();
        create_plugin(external_dir.path(), "ext-plugin");
        let ext_path = external_dir.path().join("ext-plugin");

        let plan = plan_add(
            &[ext_path.to_string_lossy().to_string()],
            &config,
            "abort",
            false,
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 1);
        assert!(plan.actions[0].is_external);

        let added = execute_add(&plan, &config, false).unwrap();
        assert_eq!(added, vec!["ext-plugin"]);

        // Plugin should be copied to pluginRoot
        let copied = config.plugin_root_abs.join("ext-plugin");
        assert!(copied.exists());
        assert!(copied.join(".claude-plugin").join("plugin.json").exists());

        // Marketplace should reference it
        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 1);
        assert_eq!(mp.plugins[0].source, "ext-plugin");
    }

    #[test]
    fn external_plugin_no_copy() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(&tmp, "");

        // Create plugin outside pluginRoot
        let external_dir = TempDir::new().unwrap();
        create_plugin(external_dir.path(), "ext-plugin");
        let ext_path = external_dir.path().join("ext-plugin");

        let plan = plan_add(
            &[ext_path.to_string_lossy().to_string()],
            &config,
            "abort",
            true, // no_copy
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 1);
        assert!(plan.actions[0].is_external);
        // Source should be absolute path since no_copy is true
        assert!(plan.actions[0].source.starts_with('/'));
    }

    #[test]
    fn add_multiple_plugins() {
        let tmp = TempDir::new().unwrap();
        let config = setup_marketplace(&tmp, "");
        create_plugin(&config.plugin_root_abs, "plugin-a");
        create_plugin(&config.plugin_root_abs, "plugin-b");

        let plan = plan_add(
            &["plugin-a".to_string(), "plugin-b".to_string()],
            &config,
            "abort",
            false,
        )
        .unwrap();

        assert_eq!(plan.actions.len(), 2);

        let added = execute_add(&plan, &config, false).unwrap();
        assert_eq!(added.len(), 2);

        let content = fs::read_to_string(&config.marketplace_path).unwrap();
        let mp: Marketplace = serde_json::from_str(&content).unwrap();
        assert_eq!(mp.plugins.len(), 2);
    }
}
