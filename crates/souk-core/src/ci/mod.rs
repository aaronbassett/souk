//! CI integration module.
//!
//! Provides hook execution (pre-commit/pre-push with git-aware staged change
//! detection), hook installation for various git hook managers, and CI
//! workflow installation for various CI/CD providers.

pub mod hooks;
pub mod install_hooks;
pub mod install_workflows;

pub use hooks::{detect_changed_plugins, is_marketplace_staged, run_pre_commit, run_pre_push};
