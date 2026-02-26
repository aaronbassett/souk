use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "souk", version, about = "Plugin marketplace management CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output machine-readable JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-error output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Color mode
    #[arg(long, global = true, default_value = "auto")]
    pub color: ColorMode,

    /// Path to marketplace.json (overrides auto-discovery)
    #[arg(long, global = true)]
    pub marketplace: Option<String>,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Validate plugins or marketplace
    Validate {
        #[command(subcommand)]
        target: ValidateTarget,
    },

    /// Add plugins to the marketplace
    Add {
        /// Plugin paths to add
        plugins: Vec<String>,

        /// Conflict resolution strategy
        #[arg(long, value_enum, default_value = "abort")]
        on_conflict: ConflictStrategy,

        /// Preview changes without executing
        #[arg(long)]
        dry_run: bool,

        /// Don't copy external plugins to pluginRoot
        #[arg(long)]
        no_copy: bool,
    },

    /// Remove plugins from the marketplace
    Remove {
        /// Plugin names to remove
        plugins: Vec<String>,

        /// Also delete plugin directory from disk
        #[arg(long)]
        delete: bool,

        /// Allow deleting plugin directories outside pluginRoot
        #[arg(long, requires = "delete")]
        allow_external_delete: bool,
    },

    /// Update plugin metadata and bump version
    Update {
        /// Plugin names to update
        plugins: Vec<String>,

        /// Bump major version
        #[arg(long, group = "bump")]
        major: bool,

        /// Bump minor version
        #[arg(long, group = "bump")]
        minor: bool,

        /// Bump patch version
        #[arg(long, group = "bump")]
        patch: bool,
    },

    /// AI-powered review
    Review {
        #[command(subcommand)]
        target: ReviewTarget,
    },

    /// CI hook management
    Ci {
        #[command(subcommand)]
        action: CiAction,
    },

    /// Scaffold a new marketplace
    Init {
        /// Directory to create marketplace in
        #[arg(long)]
        path: Option<String>,

        /// Custom plugin root directory name
        #[arg(long, default_value = "./plugins")]
        plugin_root: String,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand, Debug)]
pub enum ValidateTarget {
    /// Validate one or more plugins
    Plugin {
        /// Plugin names or paths (omit for all)
        plugins: Vec<String>,
    },
    /// Validate the marketplace
    Marketplace {
        /// Skip validating individual plugins
        #[arg(long)]
        skip_plugins: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ReviewTarget {
    /// Review a plugin
    Plugin {
        /// Plugin name or path
        plugin: String,
        #[arg(long)]
        output_dir: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    /// Review skills in a plugin
    Skill {
        /// Plugin name or path
        plugin: String,
        /// Skill names (comma-separated, omit for interactive)
        skills: Vec<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        output_dir: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
    /// Review the entire marketplace
    Marketplace {
        #[arg(long)]
        output_dir: Option<String>,
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        model: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CiAction {
    /// Run CI validation
    Run {
        #[command(subcommand)]
        hook: CiHook,
    },
    /// Install CI integration
    Install {
        #[command(subcommand)]
        target: CiInstallTarget,
    },
}

#[derive(Subcommand, Debug)]
pub enum CiHook {
    /// Run pre-commit validation
    PreCommit,
    /// Run pre-push validation
    PrePush,
}

#[derive(Subcommand, Debug)]
pub enum CiInstallTarget {
    /// Install git hooks
    Hooks {
        #[arg(long)]
        native: bool,
        #[arg(long)]
        lefthook: bool,
        #[arg(long)]
        husky: bool,
        #[arg(long)]
        overcommit: bool,
        #[arg(long)]
        hk: bool,
        #[arg(long)]
        simple_git_hooks: bool,
    },
    /// Install CI workflows
    Workflows {
        #[arg(long)]
        github: bool,
        #[arg(long)]
        blacksmith: bool,
        #[arg(long)]
        northflank: bool,
        #[arg(long)]
        circleci: bool,
        #[arg(long)]
        gitlab: bool,
        #[arg(long)]
        buildkite: bool,
    },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ConflictStrategy {
    Abort,
    Skip,
    Replace,
    Rename,
}
