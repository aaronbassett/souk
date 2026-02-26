# Safety Improvements Design

**Date**: 2026-02-26
**Approach**: Guard Rails Everywhere (validate-before-mutate + extended AtomicGuard)
**Scope**: All 7 items across Critical, Important, and Suggestion tiers

---

## 1. Delete Guard for `remove --delete` (Critical)

**Problem**: `remove_plugins` calls `fs::remove_dir_all` on whatever path `resolve_source` returns, including absolute paths outside `plugin_root_abs`.

**Changes**:

- **New CLI flag**: `--allow-external-delete` on the `Remove` clap struct. Thread through `run_remove` to `remove_plugins(names, delete_files, allow_external_delete, config)`.
- **Path guard**: After resolving the source path, canonicalize both the resolved path and `plugin_root_abs`. Reject deletion if the resolved path is not under `plugin_root_abs` and `allow_external_delete` is false.
- **Reorder operations**: Move directory deletion to after marketplace.json write + validation succeeds. If validation fails, no directory is deleted.
- **Failure policy**: If directory deletion fails after successful marketplace update, report the error but don't roll back the marketplace entry.

**Files**: `crates/souk/src/cli.rs`, `crates/souk/src/commands/remove.rs`, `crates/souk-core/src/ops/remove.rs`

**Tests**:
- External plugin + `--delete` without flag: error, no files touched
- External plugin + `--delete --allow-external-delete`: succeeds
- Internal plugin + `--delete`: succeeds as before
- Validation failure: no directory deleted, marketplace.json unchanged

---

## 2. Transactional `update` for `plugin.json` Edits (Critical)

**Problem**: `update_plugins` writes bumped versions to `plugin.json` before creating the `AtomicGuard` for `marketplace.json`. If validation fails, marketplace.json rolls back but plugin.json files retain bumped versions.

**Changes**:

- **Multiple AtomicGuards**: Create one guard per `plugin.json` being modified, plus one for `marketplace.json`. All guards created before any writes begin.
- **Reordered operation sequence**:
  1. Resolve all plugin paths (fail fast if any missing)
  2. Create AtomicGuard for marketplace.json
  3. Create AtomicGuard for each plugin.json to be modified
  4. Write bumped versions to plugin.json files
  5. Re-read plugin.json, update marketplace entries
  6. Run validate_plugin on each updated plugin
  7. Write marketplace.json
  8. Run validate_marketplace
  9. If all pass: commit all guards
  10. If any fail: drop all guards (automatic restore)

**Files**: `crates/souk-core/src/ops/update.rs`

**Tests**:
- Version bump + validation failure: all plugin.json and marketplace.json restored
- Version bump + success: all files updated, guards committed
- Rename collision: error before any writes, all files unchanged

---

## 3. Rename Collision Detection in `update` (Important)

**Problem**: If a plugin.json contains a name change after update, it could collide with an existing marketplace entry.

**Changes**:

- Before writing bumped versions, read current `plugin.json` names for each target.
- After bumping, check if any updated names collide with existing marketplace entries (excluding the entries being updated themselves).
- Fail before writing if a collision is detected.

**Files**: `crates/souk-core/src/ops/update.rs`

**Tests**:
- Update with conflicting name: clear error, all files unchanged

---

## 4. Rollback for `add` Copy Failures (Important)

**Problem**: If marketplace write or validation fails after external plugins are copied, the AtomicGuard restores marketplace.json but copied directories remain as orphans.

**Changes**:

- **Validate-before-copy**: Source plugin is already validated in `plan_add` before copy. This remains.
- **Best-effort cleanup**: Track copied directories in a `Vec<PathBuf>`. If marketplace validation fails, remove copied directories (best-effort).
- **Replace strategy**: Old dir deleted before copy. If copy fails (I/O error), report the error clearly. Don't attempt to reconstruct old dir.

**Files**: `crates/souk-core/src/ops/add.rs`

**Tests**:
- Add with validation failure after copy: copied dir cleaned up, marketplace unchanged
- Add with Replace strategy: old dir removed, new dir copied, marketplace updated

---

## 5. Symlink Detection in `copy_dir_recursive` (Suggestion)

**Problem**: `copy_dir_recursive` doesn't handle symlinks. Copying a symlink could produce unexpected behavior.

**Changes**:

- Use `fs::symlink_metadata` before processing each entry. If a symlink is detected, return an error immediately:
  ```
  "Symlink detected at '{path}': symlinks are not supported in plugin directories"
  ```
- This aborts the entire add operation. Since copy runs before marketplace write, no cleanup is needed for fresh adds.

**Files**: `crates/souk-core/src/ops/add.rs`

**Tests**:
- Add plugin directory containing a symlink: error, no marketplace changes

---

## 6. AtomicGuard Backup Collision Prevention (Suggestion)

**Problem**: Two `AtomicGuard::new()` calls on the same file within the same second produce the same `.bak.{epoch}` path. The second backup silently overwrites the first.

**Changes**:

- Change backup filename to `{stem}.{ext}.bak.{nanos}.{pid}` using `as_nanos()` and `std::process::id()`. No new dependency needed.
- Change the `Drop` impl to log a warning via `eprintln!` when restore fails, instead of silently discarding errors.

**Files**: `crates/souk-core/src/ops/atomic.rs`

**Tests**:
- Create two guards on the same file in rapid succession: unique backup paths

---

## 7. Consistent Error Handling for Source-of-Truth Drift (Suggestion)

**Problem**: Inconsistent treatment of filesystem/marketplace drift across operations.

**Changes**:

- **Filesystem-only plugins** (dir exists, no marketplace entry): **warning**. User may be developing an unregistered plugin.
- **Marketplace-only plugins** (entry exists, dir missing): **error** when the operation requires disk access (remove --delete, update --bump). Include actionable guidance: "Plugin 'foo' is listed in marketplace.json but directory not found at '...'. Run `souk remove foo` to clean up the stale entry."
- **Non-disk operations**: keep as warning for marketplace-only entries.

**Files**: `crates/souk-core/src/validation/marketplace.rs`, `crates/souk-core/src/ops/remove.rs`, `crates/souk-core/src/ops/update.rs`

**Tests**:
- Marketplace entry with missing dir + `remove --delete`: clear error with remediation hint
- Dir exists without marketplace entry: warning only, operation continues

---

## Design Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Delete guard UX | Flag only, no interactive prompt | Script-friendly, explicit flag is sufficient intent signal |
| Rollback strategy | Validate-before-mutate + best-effort cleanup | Pragmatic: avoids expensive dir backups, accepts deleted dirs are gone |
| Symlink policy | Error and abort | Clear failure mode, user must resolve before adding |
| AtomicGuard collision fix | Nanos + PID suffix | No new dependency, sufficient uniqueness |
| Multi-file atomicity | Multiple AtomicGuard instances | Simpler than extending AtomicGuard to multi-file |
