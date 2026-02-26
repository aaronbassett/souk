mod cli;
mod commands;
mod output;

use clap::Parser;
use cli::{Cli, ColorMode, Commands, ValidateTarget};
use output::{OutputMode, Reporter};

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
        _ => {
            reporter.error("Command not yet implemented");
            false
        }
    };

    reporter.finish();

    if !success {
        std::process::exit(1);
    }
}
