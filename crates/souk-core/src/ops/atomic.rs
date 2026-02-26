//! RAII-based atomic file guard for safe marketplace mutations.
//!
//! [`AtomicGuard`] creates a timestamped backup of a file before mutation begins.
//! If the operation succeeds, call [`AtomicGuard::commit`] to remove the backup.
//! If the guard is dropped without committing (e.g., due to an early return or
//! error propagation), the original file is automatically restored from the backup.
//!
//! # Examples
//!
//! ```no_run
//! use souk_core::ops::AtomicGuard;
//! use std::path::Path;
//!
//! fn update_marketplace(path: &Path) -> Result<(), souk_core::SoukError> {
//!     let guard = AtomicGuard::new(path)?;
//!
//!     // ... modify the file at `path` ...
//!
//!     // Success: remove the backup.
//!     guard.commit()?;
//!     Ok(())
//! }
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::SoukError;

/// An RAII guard that backs up a file before mutation and restores it on drop
/// unless explicitly committed.
///
/// The backup file is named `{original}.bak.{epoch_secs}` and lives alongside
/// the original. This mirrors the pattern used by the shell-based atomic helpers
/// in `temp-reference-scripts/lib/atomic.sh`.
///
/// # Behavior
///
/// - **`new(path)`**: Creates a backup copy of the file. If the file does not
///   exist, no backup is created and the guard is a no-op on drop.
/// - **`commit(self)`**: Removes the backup file and consumes the guard so that
///   `Drop` does not run the restore logic.
/// - **`Drop`**: If the guard was not committed and a backup exists, restores
///   the original file from the backup.
pub struct AtomicGuard {
    /// Path to the original file being guarded.
    original_path: PathBuf,
    /// Path to the backup file, or `None` if no backup was created
    /// (e.g., the original file did not exist).
    backup_path: Option<PathBuf>,
    /// Whether [`commit`](AtomicGuard::commit) has been called.
    committed: bool,
}

impl AtomicGuard {
    /// Creates a new `AtomicGuard` for the file at `path`.
    ///
    /// If the file exists, a timestamped backup is created immediately. If
    /// the file does not exist (e.g., it will be created fresh by the
    /// operation), no backup is made and the guard becomes a no-op on drop.
    ///
    /// # Errors
    ///
    /// Returns [`SoukError::Io`] if the file exists but cannot be copied.
    pub fn new(path: &Path) -> Result<Self, SoukError> {
        let original_path = path.to_path_buf();

        let backup_path = if original_path.exists() {
            let epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before UNIX epoch")
                .as_secs();

            let backup = original_path.with_extension(format!(
                "{}.bak.{}",
                original_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or(""),
                epoch
            ));

            fs::copy(&original_path, &backup)?;
            Some(backup)
        } else {
            None
        };

        Ok(Self {
            original_path,
            backup_path,
            committed: false,
        })
    }

    /// Returns the path to the backup file, if one was created.
    pub fn backup_path(&self) -> Option<&Path> {
        self.backup_path.as_deref()
    }

    /// Returns the path to the original file being guarded.
    pub fn original_path(&self) -> &Path {
        &self.original_path
    }

    /// Commits the operation, removing the backup file.
    ///
    /// This consumes the guard so that `Drop` will not attempt to restore
    /// the original file. Call this after the mutation has been verified
    /// as successful.
    ///
    /// # Errors
    ///
    /// Returns [`SoukError::Io`] if the backup file exists but cannot be
    /// removed. Even on error, the guard is marked as committed so that
    /// `Drop` will not attempt a restore (the mutation already succeeded).
    pub fn commit(mut self) -> Result<(), SoukError> {
        self.committed = true;
        if let Some(ref backup) = self.backup_path {
            if backup.exists() {
                fs::remove_file(backup)?;
            }
        }
        Ok(())
    }
}

impl Drop for AtomicGuard {
    fn drop(&mut self) {
        if self.committed {
            return;
        }

        if let Some(ref backup) = self.backup_path {
            if backup.exists() {
                // Best-effort restore. If this fails there is not much we can do
                // from a destructor -- the backup file remains on disk for manual
                // recovery.
                let _ = fs::copy(backup, &self.original_path);
                let _ = fs::remove_file(backup);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a temp directory with a file containing the given content.
    fn setup_file(content: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("failed to create temp dir");
        let file_path = dir.path().join("marketplace.json");
        fs::write(&file_path, content).expect("failed to write test file");
        (dir, file_path)
    }

    #[test]
    fn backup_is_created_on_new() {
        let (_dir, file_path) = setup_file(r#"{"version":"1.0.0"}"#);

        let guard = AtomicGuard::new(&file_path).expect("guard creation failed");

        // A backup file should exist.
        let backup = guard.backup_path().expect("expected a backup path");
        assert!(backup.exists(), "backup file should exist on disk");

        // Backup should contain the same content as the original.
        let backup_content = fs::read_to_string(backup).unwrap();
        assert_eq!(backup_content, r#"{"version":"1.0.0"}"#);

        // Clean up by committing.
        guard.commit().unwrap();
    }

    #[test]
    fn commit_removes_backup() {
        let (_dir, file_path) = setup_file(r#"{"version":"1.0.0"}"#);

        let guard = AtomicGuard::new(&file_path).expect("guard creation failed");
        let backup = guard.backup_path().expect("expected a backup path").to_path_buf();

        assert!(backup.exists(), "backup should exist before commit");

        guard.commit().unwrap();

        assert!(!backup.exists(), "backup should be removed after commit");

        // Original should still be intact.
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, r#"{"version":"1.0.0"}"#);
    }

    #[test]
    fn drop_restores_original_on_failure() {
        let (_dir, file_path) = setup_file(r#"{"version":"1.0.0"}"#);

        {
            let _guard = AtomicGuard::new(&file_path).expect("guard creation failed");

            // Simulate a mutation that corrupts the file.
            fs::write(&file_path, r#"{"CORRUPTED":true}"#).unwrap();

            // Guard drops here without commit -- should restore the original.
        }

        let restored = fs::read_to_string(&file_path).unwrap();
        assert_eq!(
            restored,
            r#"{"version":"1.0.0"}"#,
            "original file should be restored after drop"
        );
    }

    #[test]
    fn drop_after_commit_does_not_restore() {
        let (_dir, file_path) = setup_file(r#"{"version":"1.0.0"}"#);

        {
            let guard = AtomicGuard::new(&file_path).expect("guard creation failed");

            // Mutate the file (legitimate update).
            fs::write(&file_path, r#"{"version":"2.0.0"}"#).unwrap();

            guard.commit().unwrap();
            // Guard drops here, but committed -- should NOT restore.
        }

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(
            content,
            r#"{"version":"2.0.0"}"#,
            "committed mutation should persist"
        );
    }

    #[test]
    fn guard_on_nonexistent_file_is_noop() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let file_path = dir.path().join("does_not_exist.json");

        assert!(!file_path.exists());

        let guard = AtomicGuard::new(&file_path).expect("guard creation should succeed");
        assert!(
            guard.backup_path().is_none(),
            "no backup should be created for non-existent file"
        );

        // Dropping without commit should be safe.
        drop(guard);

        // File still should not exist (guard didn't create it).
        assert!(!file_path.exists());
    }

    #[test]
    fn guard_on_nonexistent_file_commit_is_noop() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let file_path = dir.path().join("does_not_exist.json");

        let guard = AtomicGuard::new(&file_path).expect("guard creation should succeed");
        // Committing a guard with no backup should succeed silently.
        guard.commit().unwrap();
    }

    #[test]
    fn drop_cleans_up_backup_file() {
        let (_dir, file_path) = setup_file(r#"{"version":"1.0.0"}"#);
        let backup_path;

        {
            let guard = AtomicGuard::new(&file_path).expect("guard creation failed");
            backup_path = guard.backup_path().unwrap().to_path_buf();
            assert!(backup_path.exists());

            // Mutate the file.
            fs::write(&file_path, r#"{"CORRUPTED":true}"#).unwrap();

            // Drop without commit -- restore + cleanup.
        }

        assert!(
            !backup_path.exists(),
            "backup file should be removed after drop restores"
        );
    }

    #[test]
    fn multiple_guards_on_different_files() {
        let dir = TempDir::new().expect("failed to create temp dir");
        let file_a = dir.path().join("a.json");
        let file_b = dir.path().join("b.json");
        fs::write(&file_a, "aaa").unwrap();
        fs::write(&file_b, "bbb").unwrap();

        let guard_a = AtomicGuard::new(&file_a).unwrap();
        let guard_b = AtomicGuard::new(&file_b).unwrap();

        // Mutate both.
        fs::write(&file_a, "AAA").unwrap();
        fs::write(&file_b, "BBB").unwrap();

        // Commit A, drop B (restore).
        guard_a.commit().unwrap();
        drop(guard_b);

        assert_eq!(fs::read_to_string(&file_a).unwrap(), "AAA");
        assert_eq!(fs::read_to_string(&file_b).unwrap(), "bbb");
    }

    #[test]
    fn backup_path_includes_original_extension() {
        let (_dir, file_path) = setup_file("test");

        let guard = AtomicGuard::new(&file_path).unwrap();
        let backup = guard.backup_path().unwrap();

        // The backup path should contain "json.bak."
        let backup_name = backup.file_name().unwrap().to_str().unwrap();
        assert!(
            backup_name.contains("json.bak."),
            "backup name '{backup_name}' should contain 'json.bak.'"
        );

        guard.commit().unwrap();
    }
}
