//! CI hook support for pre-commit and pre-push validation.
//!
//! This module detects staged changes via git and runs targeted validation
//! for pre-commit hooks (only changed plugins) or full validation for
//! pre-push hooks (entire marketplace).

pub mod hooks;

pub use hooks::{detect_changed_plugins, is_marketplace_staged, run_pre_commit, run_pre_push};
