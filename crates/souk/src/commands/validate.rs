use std::path::PathBuf;

use souk_core::discovery::{discover_marketplace, load_marketplace_config, MarketplaceConfig};
use souk_core::resolution::resolve_plugin;
use souk_core::validation::{validate_marketplace, validate_plugin};

use crate::output::Reporter;

pub fn run_validate_plugin(
    plugins: &[String],
    marketplace_override: Option<&str>,
    reporter: &mut Reporter,
) -> bool {
    let config = load_config(marketplace_override);

    let plugin_paths = collect_plugin_paths(plugins, config.as_ref(), reporter);

    if plugin_paths.is_empty() {
        reporter.error("No plugins found to validate");
        return false;
    }

    reporter.section(&format!("Validating {} plugin(s)", plugin_paths.len()));

    let mut success_count = 0;
    let mut failure_count = 0;

    for path in &plugin_paths {
        let plugin_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());

        let result = validate_plugin(path);

        if result.has_errors() {
            failure_count += 1;
            reporter.report_validation(&result);
        } else {
            success_count += 1;
            reporter.success_with_details(
                &format!("Plugin validated: {plugin_name}"),
                &format!("path: {}", path.display()),
            );
            if result.warning_count() > 0 {
                reporter.report_validation(&result);
            }
        }
    }

    reporter.section("Summary");
    reporter.info(&format!(
        "{} plugin(s): {success_count} passed, {failure_count} failed",
        plugin_paths.len()
    ));

    failure_count == 0
}

pub fn run_validate_marketplace(
    skip_plugins: bool,
    marketplace_override: Option<&str>,
    reporter: &mut Reporter,
) -> bool {
    let config = match load_config_required(marketplace_override, reporter) {
        Some(c) => c,
        None => return false,
    };

    reporter.section("Validating marketplace");

    let result = validate_marketplace(&config, skip_plugins);

    reporter.report_validation(&result);

    if result.has_errors() {
        reporter.error("Marketplace validation failed");
        false
    } else {
        reporter.success("Marketplace validation passed");
        true
    }
}

fn load_config(marketplace_override: Option<&str>) -> Option<MarketplaceConfig> {
    let mp_path = if let Some(path) = marketplace_override {
        PathBuf::from(path)
    } else {
        let cwd = std::env::current_dir().ok()?;
        discover_marketplace(&cwd).ok()?
    };
    load_marketplace_config(&mp_path).ok()
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

fn collect_plugin_paths(
    plugins: &[String],
    config: Option<&MarketplaceConfig>,
    reporter: &mut Reporter,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if plugins.is_empty() {
        let Some(config) = config else {
            reporter.error("No marketplace found and no plugins specified");
            return paths;
        };
        let plugin_root = &config.plugin_root_abs;
        if !plugin_root.is_dir() {
            reporter.error(&format!(
                "Plugin root directory not found: {}",
                plugin_root.display()
            ));
            return paths;
        }

        if let Ok(entries) = std::fs::read_dir(plugin_root) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    paths.push(entry.path());
                }
            }
        }
        paths.sort();
    } else {
        for input in plugins {
            let input_path = PathBuf::from(input);

            if input_path.is_dir() {
                if input_path.join(".claude-plugin").join("plugin.json").is_file() {
                    paths.push(input_path);
                } else if let Ok(entries) = std::fs::read_dir(&input_path) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir()
                            && entry
                                .path()
                                .join(".claude-plugin")
                                .join("plugin.json")
                                .is_file()
                        {
                            paths.push(entry.path());
                        }
                    }
                }
            } else {
                match resolve_plugin(input, config) {
                    Ok(p) => paths.push(p),
                    Err(_) => {
                        reporter.error(&format!("Plugin not found: {input}"));
                    }
                }
            }
        }
    }

    paths
}
