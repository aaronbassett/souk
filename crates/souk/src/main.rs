mod cli;
mod commands;
mod output;

use std::path::PathBuf;

use clap::Parser;
use cli::{CiAction, CiHook, Cli, ColorMode, Commands, ReviewTarget, ValidateTarget};
use output::{OutputMode, Reporter};
use souk_core::discovery::{discover_marketplace, load_marketplace_config, MarketplaceConfig};

fn main() {
    let cli = Cli::parse();

    let mode = if cli.json {
        OutputMode::Json
    } else if cli.quiet {
        OutputMode::Quiet
    } else {
        OutputMode::Human
    };

    match cli.color {
        ColorMode::Never => colored::control::set_override(false),
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Auto => {}
    }

    let mut reporter = Reporter::new(mode);
    let marketplace = cli.marketplace.as_deref();

    let success = match cli.command {
        Commands::Validate { target } => match target {
            ValidateTarget::Plugin { plugins } => {
                commands::validate::run_validate_plugin(&plugins, marketplace, &mut reporter)
            }
            ValidateTarget::Marketplace { skip_plugins } => {
                commands::validate::run_validate_marketplace(
                    skip_plugins,
                    marketplace,
                    &mut reporter,
                )
            }
        },
        Commands::Init { path, plugin_root } => {
            let target = path.as_deref().unwrap_or(".");
            commands::init::run_init(target, &plugin_root, &mut reporter)
        }
        Commands::Add {
            plugins,
            on_conflict,
            dry_run,
            no_copy,
        } => match load_config_required(marketplace, &mut reporter) {
            Some(config) => commands::add::run_add(
                &plugins,
                &on_conflict,
                dry_run,
                no_copy,
                &config,
                &mut reporter,
            ),
            None => false,
        },
        Commands::Remove {
            plugins,
            delete,
            allow_external_delete,
        } => match load_config_required(marketplace, &mut reporter) {
            Some(config) => commands::remove::run_remove(
                &plugins,
                delete,
                allow_external_delete,
                &config,
                &mut reporter,
            ),
            None => false,
        },
        Commands::Update {
            plugins,
            major,
            minor,
            patch,
        } => {
            let bump_type = if major {
                Some("major")
            } else if minor {
                Some("minor")
            } else if patch {
                Some("patch")
            } else {
                None
            };
            match load_config_required(marketplace, &mut reporter) {
                Some(config) => {
                    commands::update::run_update(&plugins, bump_type, &config, &mut reporter)
                }
                None => false,
            }
        }
        Commands::Review { target } => match target {
            ReviewTarget::Plugin {
                plugin,
                output_dir,
                provider,
                model,
            } => commands::review::run_review_plugin(
                &plugin,
                output_dir.as_deref(),
                provider.as_deref(),
                model.as_deref(),
                marketplace,
                &mut reporter,
            ),
            _ => {
                reporter.error("Review subcommand not yet implemented");
                false
            }
        },
        Commands::Ci { action } => match action {
            CiAction::Run { hook } => match hook {
                CiHook::PreCommit => commands::ci::run_pre_commit(marketplace, &mut reporter),
                CiHook::PrePush => commands::ci::run_pre_push(marketplace, &mut reporter),
            },
            CiAction::Install { target } => commands::ci::run_ci_install(&target, &mut reporter),
        },
        Commands::Prune { apply } => match load_config_required(marketplace, &mut reporter) {
            Some(config) => commands::prune::run_prune(apply, &config, &mut reporter),
            None => false,
        },
        Commands::Completions { shell } => {
            use clap::CommandFactory;
            clap_complete::generate(
                shell,
                &mut cli::Cli::command(),
                "souk",
                &mut std::io::stdout(),
            );
            true
        }
    };

    reporter.finish();

    if !success {
        std::process::exit(1);
    }
}

/// Loads the marketplace configuration, reporting an error if it cannot be found.
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
