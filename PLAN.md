# Souk: Rust CLI for Plugin Marketplace Management

## Context

The midnight-expert project has ~2,370 lines across 13 shell scripts managing a Claude Code plugin marketplace. These scripts handle validation, plugin management, AI-powered reviews, and CI hooks. They've reached their complexity ceiling: fragile error handling, no test coverage, limited functionality sharing between scripts, and poor extensibility. Souk replaces all scripts with a proper Rust CLI, distributed as a standalone binary from a new repository.

## Command Tree

```
souk
├── validate
│   ├── plugin <path>...                         # deterministic ONLY, no Claude CLI
│   └── marketplace [--skip-plugins]              # deterministic ONLY, no Claude CLI
├── add <path>... [--conflict skip|replace|rename] [--dry-run] [--no-copy]
├── review
│   ├── plugin <name> [--output-dir]              # uses Claude CLI
│   ├── skill <plugin> [skill] [--all]            # uses Claude CLI
│   └── marketplace [--output-dir]                # uses Claude CLI (NEW)
├── ci
│   ├── run
│   │   ├── pre-commit
│   │   └── pre-push
│   └── install
│       ├── hooks [--native|--lefthook|--husky|--overcommit|--hk|--simple-git-hooks]
│       └── workflows [--github|--blacksmith|--northflank|--circleci|--gitlab|--buildkite]
└── init                                          # scaffold new marketplace
```

**Global flags:** `--json`, `--quiet`, `--color auto|always|never`, `--marketplace <path>`

## Architecture

Cargo workspace with two crates in a **new repository**:

- **`souk-core`** - Library crate. Pure domain logic, no CLI concerns, no output formatting. Returns `Result<T, SoukError>` everywhere. Other tools can depend on this.
- **`souk`** - CLI binary crate. Thin layer: parses args with clap, calls core, formats output.

### Repository Structure

```
souk/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── souk-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs          # thiserror error types + ValidationDiagnostic
│   │       ├── types/
│   │       │   ├── marketplace.rs  # Marketplace, PluginEntry, Owner
│   │       │   ├── plugin.rs       # PluginManifest, Author
│   │       │   ├── skill.rs        # SkillMetadata
│   │       │   └── version_constraint.rs
│   │       ├── resolution/
│   │       │   ├── plugin.rs       # 3-level plugin resolution
│   │       │   └── skill.rs        # 3-level skill resolution + frontmatter parsing
│   │       ├── validation/
│   │       │   ├── plugin.rs       # deterministic plugin validation
│   │       │   ├── marketplace.rs  # deterministic marketplace + completeness checks
│   │       │   └── extends.rs      # extends-plugin.json schema validation
│   │       ├── ops/
│   │       │   ├── add.rs          # plan-then-execute add pipeline
│   │       │   ├── init.rs         # marketplace scaffolding
│   │       │   └── atomic.rs       # AtomicGuard with RAII rollback
│   │       ├── review/
│   │       │   ├── claude.rs       # ClaudeRunner trait (mockable)
│   │       │   ├── plugin.rs
│   │       │   ├── skill.rs
│   │       │   └── marketplace.rs
│   │       ├── ci/
│   │       │   ├── hooks.rs        # hook manager detection + installation
│   │       │   └── workflows.rs    # CI provider detection + templates
│   │       └── version.rs          # semver bumping, unique name generation
│   └── souk/
│       └── src/
│           ├── main.rs
│           ├── cli.rs              # clap derive definitions
│           ├── output.rs           # Reporter: json/human/quiet output
│           └── commands/
│               ├── validate.rs
│               ├── add.rs
│               ├── review.rs
│               ├── ci.rs
│               └── init.rs
└── tests/
    ├── fixtures/                   # marketplace + plugin directory structures
    └── integration/
```

### Key Dependencies

| Crate | souk-core | souk (CLI) |
|---|---|---|
| serde + serde_json | x | |
| thiserror | x | |
| semver | x | |
| walkdir | x | |
| tempfile | x | |
| regex | x | |
| clap 4 (derive) | | x |
| colored | | x |
| dialoguer | | x |
| indicatif | | x |
| insta (dev) | x | x |

### Key Design Decisions

1. **Validation is deterministic only.** `souk validate` never calls Claude CLI. The `review` commands use Claude.
2. **AtomicGuard with Drop.** Automatic rollback on panic/error replaces manual backup/restore bash pattern.
3. **Plan-then-execute for add.** `plan_add()` returns an `AddPlan`, `execute_add()` takes it. Dry-run = print the plan.
4. **ClaudeRunner trait.** All Claude CLI calls go through a trait, allowing mock implementations in tests.
5. **Reporter abstraction.** Unified output layer handles `--json`/`--quiet`/`--color` globally.
6. **Upward marketplace discovery.** Searches parent directories for `.claude-plugin/marketplace.json`, stops at git root. `--marketplace` flag overrides.

## Implementation Phases

### Phase 1: Core Foundation
Workspace skeleton, types, resolution, deterministic validation, CLI scaffolding.

**Deliverables:** Working `souk validate plugin` and `souk validate marketplace`

- Create workspace Cargo.toml and both crate Cargo.toml files
- Define serde types for marketplace.json, plugin.json, extends-plugin.json, SKILL.md frontmatter
- Port plugin resolution logic from `scripts/lib/marketplace.sh:resolve_plugin_to_path`
- Port skill resolution logic from `scripts/review-skill.sh:resolve_skill`
- Port deterministic validation from `scripts/ci/validate.sh` and `scripts/lib/validation.sh`
  - Plugin: directory structure, JSON syntax, required fields, valid semver
  - Marketplace: JSON syntax, plugins field, completeness check, duplicate names
  - Extends-plugin: allowed keys, version constraint regex validation
- Build clap CLI with `validate plugin` and `validate marketplace` subcommands
- Build Reporter (output.rs) with JSON/human/quiet modes
- Test fixtures from actual midnight-expert marketplace structure
- Unit tests for all validation rules, resolution paths
- Snapshot tests (insta) for CLI output

### Phase 2: Plugin Management
Add command, init command, atomic operations, version bumping.

**Deliverables:** Working `souk add` and `souk init`

- Implement AtomicGuard (RAII backup/restore using tempfile + Drop)
- Port add pipeline from `scripts/add-plugin.sh` (7 phases):
  - Preflight validation, plan operations, dry-run report, copy, atomic marketplace update, version bump, final validation
- Port conflict strategies: skip, replace, rename (with `generate_unique_name`)
- Port version bumping from `scripts/lib/atomic.sh:bump_marketplace_minor_version`
- Implement init command (scaffold marketplace directory structure)
- Integration tests for add with all conflict strategies

### Phase 3: Review Commands
Claude CLI integration with trait-based abstraction.

**Deliverables:** Working `souk review plugin|skill|marketplace`

- Define ClaudeRunner trait + RealClaudeRunner implementation
- Port review-plugin prompt from `scripts/review-plugin.sh`
- Port review-skill logic from `scripts/review-skill.sh` including:
  - Interactive skill selection with dialoguer
  - Batch review with `--all`
  - Per-skill output directory organization
- Implement new `souk review marketplace` command
- MockClaudeRunner for testing
- Tests verify prompt construction and output file organization

### Phase 4: CI Commands
Hook management, workflow generation, auto-detection.

**Deliverables:** Working `souk ci run` and `souk ci install`

- **`souk ci run pre-commit`**: Port from `scripts/ci/commit.sh`
  - Detect changed plugins from `git diff --cached`
  - Validate only changed plugins + marketplace if staged
- **`souk ci run pre-push`**: Port from `scripts/ci/push.sh`
  - Full marketplace validation
- **`souk ci install hooks`**: Auto-detect hook manager
  - Detection: check local config files (lefthook.yml, .husky/, .overcommit.yml, hk.toml, .simple-git-hooks.json)
  - Fallback: check global installs (`which lefthook`, etc.)
  - Default: native git hooks
  - Confirm choice with user via dialoguer
  - Generate per-manager config (native scripts, lefthook.yml snippet, .husky/ files, etc.)
- **`souk ci install workflows`**: Auto-detect CI provider
  - Detection: existing config files (.github/workflows, .circleci/, .gitlab-ci.yml, .buildkite/)
  - Fallback: CLI tools (blacksmith, glab, bk)
  - Fallback: git remote URL
  - Confirm choice with user
  - Write workflow template for selected provider
- **`souk ci install`** (bare): Prompt user to choose hooks, workflows, or both

### Phase 5: Distribution & Polish
Cross-platform builds, shell completions, error reporting.

- Cross-platform CI with `cross` (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64)
- Shell completions via `clap_complete` (`souk completions bash|zsh|fish`)
- Man pages via `clap_mangen`
- Progress bars via `indicatif` for batch operations
- Rich error display with file paths, field names, and fix suggestions
- End-to-end integration tests

## Script-to-Command Migration Map

| Current Script | Souk Command | Notes |
|---|---|---|
| `scripts/validate-plugin.sh` | `souk validate plugin` | Deterministic only (no Claude CLI) |
| `scripts/validate-marketplace.sh` | `souk validate marketplace` | Deterministic only |
| `scripts/ci/validate.sh` | `souk validate marketplace --ci` | Same deterministic logic |
| `scripts/add-plugin.sh` | `souk add` | Full 7-phase pipeline |
| `scripts/review-plugin.sh` | `souk review plugin` | Claude CLI |
| `scripts/review-skill.sh` | `souk review skill` | Claude CLI |
| `scripts/ci/commit.sh` | `souk ci run pre-commit` | Git-aware changed plugin detection |
| `scripts/ci/push.sh` | `souk ci run pre-push` | Full validation |
| *(new)* | `souk init` | Scaffold marketplace |
| *(new)* | `souk review marketplace` | Claude CLI |
| *(new)* | `souk ci install hooks` | Auto-detect + install git hooks |
| *(new)* | `souk ci install workflows` | Auto-detect + install CI workflows |

## Critical Source Files to Port From

- `scripts/lib/validation.sh` - All deterministic validation rules
- `scripts/add-plugin.sh` - 7-phase add pipeline, conflict resolution
- `scripts/lib/marketplace.sh` - Plugin/skill resolution logic
- `scripts/review-skill.sh` - Skill resolution, interactive selection, review prompts
- `scripts/lib/atomic.sh` - Backup/restore, atomic writes, version bumping
- `.claude-plugin/marketplace.json` - Canonical schema for serde types

## Verification

1. **Unit tests**: Every validation rule, resolution path, atomic operation, version bump
2. **Integration tests**: Full CLI commands against fixture directories, snapshot output with insta
3. **Parity check**: Run souk validate against the actual midnight-expert marketplace and compare results with existing scripts
4. **CI check**: Install hooks in a test repo, make a commit, verify pre-commit fires
5. **Cross-platform**: CI matrix builds on Linux, macOS, Windows

