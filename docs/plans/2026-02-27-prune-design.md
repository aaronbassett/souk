# souk prune — Design Document

## Summary

Add a `souk prune` command that removes orphaned plugin directories (directories
present on disk under `pluginRoot` but not listed in `marketplace.json`).
Default behavior is dry-run; `--apply` actually deletes.

## Decisions

- **No marketplace.json mutation** — orphans have no entry to remove, so this is
  a pure filesystem operation. No version bump.
- **No interactive prompt** — `--apply` flag is sufficient explicit intent.
  Clean for scripting and CI.
- **Shared orphan-detection helper** — extract `find_orphaned_dirs()` from the
  validation module's `check_completeness` logic. Both validation and prune call
  the same function.
- **Exit 0 when nothing to prune** — prints a message in human mode, silent in
  `--quiet` mode.

## CLI Interface

```
souk prune              # Dry-run: lists orphaned directories
souk prune --apply      # Actually deletes orphaned directories
```

Standard global flags: `--json`, `--quiet`, `--color`, `--marketplace`.

Clap definition:

```rust
/// Remove orphaned plugin directories not listed in marketplace.json
Prune {
    /// Actually delete orphaned directories (default: dry-run)
    #[arg(long)]
    apply: bool,
},
```

## Core Logic

### Shared helper

Extract into the validation module (or a new `souk-core/src/orphan.rs`):

```rust
/// Returns full paths of directories under pluginRoot not listed in marketplace.json.
pub fn find_orphaned_dirs(config: &MarketplaceConfig) -> Result<Vec<PathBuf>, SoukError>
```

Scans `config.plugin_root_abs`, collects directory names, diffs against
`config.marketplace.plugins[*].source`, returns full paths. The existing
`check_completeness` in validation calls this helper instead of duplicating
the scan.

### Prune operation (`souk-core/src/ops/prune.rs`)

```rust
pub struct PruneResult {
    pub orphaned: Vec<PathBuf>,   // Orphaned directories found
    pub deleted: Vec<PathBuf>,    // Directories actually deleted (empty if dry-run)
    pub warnings: Vec<String>,    // Non-fatal failures (e.g. permission denied)
}

pub fn prune_plugins(apply: bool, config: &MarketplaceConfig) -> Result<PruneResult, SoukError>
```

- Calls `find_orphaned_dirs(config)` to get the list
- If `!apply`, returns with `orphaned` populated, `deleted` empty
- If `apply`, iterates with `fs::remove_dir_all` for each orphan
- Failures collected as warnings (same pattern as `remove`)
- No `AtomicGuard` needed — no marketplace.json mutation
- No `--allow-external-delete` needed — orphans are found by scanning
  `pluginRoot`, so they're inherently within bounds

### CLI handler (`crates/souk/src/commands/prune.rs`)

```rust
pub fn run_prune(apply: bool, config: &MarketplaceConfig, reporter: &mut Reporter) -> bool
```

## Output

| Scenario              | Human                                          | JSON                        | Quiet     |
|-----------------------|------------------------------------------------|-----------------------------|-----------|
| No orphans found      | Info message                                   | Info entry                  | Silent    |
| Dry-run with orphans  | Lists each path, summary count                 | Array of orphan entries     | Silent    |
| Apply with orphans    | Lists each deleted path, summary count         | Array of deleted entries    | Silent    |
| Delete failure        | Warning per failed path                        | Warning entries             | stderr    |

Human output examples:

```
=== Prune (dry-run) ===
  Would delete: plugins/stale-plugin
  Would delete: plugins/old-experiment
Found 2 orphaned plugin directory(ies). Run with --apply to delete.
```

```
=== Prune ===
✓ Deleted: plugins/stale-plugin
✓ Deleted: plugins/old-experiment
Successfully pruned 2 orphaned plugin directory(ies).
```

## Testing

### Unit tests (`souk-core/src/ops/prune.rs`)

1. `prune_dry_run_lists_orphans` — orphaned dirs found, nothing deleted
2. `prune_apply_deletes_orphans` — orphaned dirs removed from disk
3. `prune_no_orphans` — clean marketplace returns empty result
4. `prune_partial_failure_warns` — one dir undeletable, others succeed
5. `find_orphaned_dirs_accuracy` — helper returns correct set

### Integration tests (`crates/souk/tests/`)

1. `prune_dry_run_output` — CLI output format, exit code 0
2. `prune_apply_output` — CLI output after deletion, exit code 0
3. `prune_json_output` — JSON mode produces valid structured output
4. `prune_nothing_to_do` — clean marketplace, appropriate message

### Validation refactor

Update existing `check_completeness` to call `find_orphaned_dirs()` helper.
Update existing tests to verify shared behavior.

## Files to create/modify

### New files
- `crates/souk-core/src/ops/prune.rs` — core prune operation + unit tests
- `crates/souk/src/commands/prune.rs` — CLI handler
- `crates/souk/tests/prune_test.rs` — integration tests

### Modified files
- `crates/souk-core/src/ops/mod.rs` — add `pub mod prune;`
- `crates/souk-core/src/validation/marketplace.rs` — extract `find_orphaned_dirs()`, refactor `check_completeness`
- `crates/souk/src/commands/mod.rs` — add `pub mod prune;`
- `crates/souk/src/cli.rs` — add `Prune` variant to `Commands` enum
- `crates/souk/src/main.rs` — add match arm for `Commands::Prune`
