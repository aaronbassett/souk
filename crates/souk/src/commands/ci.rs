//! CLI handler for `souk ci` subcommands.
//!
//! Handles:
//! - `souk ci run pre-commit` / `souk ci run pre-push`
//! - `souk ci install hooks [--native|--lefthook|--husky|...]`
//! - `souk ci install workflows [--github|--circleci|--gitlab|...]`

use std::env;
use std::path::PathBuf;

use souk_core::ci::install_hooks::{detect_hook_manager, install_hooks, HookManager};
use souk_core::ci::install_workflows::{detect_ci_provider, install_workflow, CiProvider};
use souk_core::discovery::{discover_marketplace, load_marketplace_config, MarketplaceConfig};

use crate::cli::CiInstallTarget;
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

/// Install CI integration (hooks or workflows).
pub fn run_ci_install(target: &CiInstallTarget, reporter: &mut Reporter) -> bool {
    let cwd = match env::current_dir() {
        Ok(c) => c,
        Err(e) => {
            reporter.error(&format!("Cannot get current directory: {e}"));
            return false;
        }
    };

    match target {
        CiInstallTarget::Hooks {
            native,
            lefthook,
            husky,
            overcommit,
            hk,
            simple_git_hooks,
        } => {
            let manager = if *native {
                HookManager::Native
            } else if *lefthook {
                HookManager::Lefthook
            } else if *husky {
                HookManager::Husky
            } else if *overcommit {
                HookManager::Overcommit
            } else if *hk {
                HookManager::Hk
            } else if *simple_git_hooks {
                HookManager::SimpleGitHooks
            } else {
                // Auto-detect
                match detect_hook_manager(&cwd) {
                    Some(m) => {
                        reporter.info(&format!("Detected hook manager: {m}"));
                        m
                    }
                    None => {
                        reporter.info(
                            "No hook manager detected, defaulting to native git hooks",
                        );
                        HookManager::Native
                    }
                }
            };

            reporter.section(&format!("Installing hooks via {manager}"));

            match install_hooks(&cwd, &manager) {
                Ok(msg) => {
                    reporter.success(&msg);
                    true
                }
                Err(e) => {
                    reporter.error(&format!("Failed to install hooks: {e}"));
                    false
                }
            }
        }
        CiInstallTarget::Workflows {
            github,
            blacksmith,
            northflank,
            circleci,
            gitlab,
            buildkite,
        } => {
            let provider = if *github {
                CiProvider::GitHub
            } else if *blacksmith {
                CiProvider::Blacksmith
            } else if *northflank {
                CiProvider::Northflank
            } else if *circleci {
                CiProvider::CircleCi
            } else if *gitlab {
                CiProvider::GitLab
            } else if *buildkite {
                CiProvider::Buildkite
            } else {
                // Auto-detect
                match detect_ci_provider(&cwd) {
                    Some(p) => {
                        reporter.info(&format!("Detected CI provider: {p}"));
                        p
                    }
                    None => {
                        reporter.info(
                            "No CI provider detected, defaulting to GitHub Actions",
                        );
                        CiProvider::GitHub
                    }
                }
            };

            reporter.section(&format!("Installing CI workflow for {provider}"));

            match install_workflow(&cwd, &provider) {
                Ok(msg) => {
                    reporter.success(&msg);
                    true
                }
                Err(e) => {
                    reporter.error(&format!("Failed to install workflow: {e}"));
                    false
                }
            }
        }
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
