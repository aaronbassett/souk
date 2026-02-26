//! Atomic operations for safe marketplace mutations.
//!
//! This module provides RAII-based backup/restore guards that ensure
//! marketplace files are never left in a corrupted state. If an operation
//! fails partway through, the guard's `Drop` implementation automatically
//! restores the original file from its backup.

pub mod atomic;
pub mod init;

pub use atomic::AtomicGuard;
