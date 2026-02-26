//! CI integration module.
//!
//! Provides hook execution, hook installation for various git hook managers,
//! and CI workflow installation for various CI/CD providers.

pub mod hooks;
pub mod install_hooks;
pub mod install_workflows;
