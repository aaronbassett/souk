<!-- Sync Impact Report
Version change: N/A → 1.0.0
Added principles:
  - I. Unix Philosophy
  - II. Library-Core Architecture
  - III. Defense in Depth
  - IV. Fail Fast with Actionable Feedback
  - V. Test Real Workflows
  - VI. Simplicity & YAGNI
  - VII. Atomic Safety
  - VIII. Continuous Delivery
Added sections:
  - Core Principles (8 principles)
  - Rust Development Standards
  - Development Workflow
  - Governance
Removed sections: N/A (initial creation)
Templates requiring updates: N/A (no prior constitution)
Follow-up TODOs: None
-->

# Souk Constitution

## Core Principles

### I. Unix Philosophy

Souk is a composable CLI tool. Each command does one thing well.

- Text I/O protocol: arguments and stdin for input, stdout for results,
  stderr for diagnostics.
- Support both `--json` and human-readable output formats on every command
  that produces structured data.
- Exit codes MUST be predictable and documented: 0 for success, 1 for
  validation failure, 2 for usage error, non-zero for all other failures.
- Commands MUST be composable — output of one command usable as input to
  scripts, pipelines, and other tools.

**Rationale:** Plugin authors and CI systems depend on predictable,
scriptable behavior. Surprising output or exit codes break automation.

### II. Library-Core Architecture

`souk-core` is the product. `souk` (CLI) is a thin presentation layer.

- ALL domain logic — validation, resolution, add pipeline, review
  orchestration, CI detection — lives in `souk-core`.
- `souk-core` MUST NOT depend on CLI concerns (clap, colored, dialoguer,
  indicatif). It returns `Result<T, SoukError>` and nothing else.
- The CLI crate handles argument parsing, output formatting, and user
  interaction. It calls `souk-core` functions and formats their results.
- Other tools MAY depend on `souk-core` as a library without pulling in
  CLI dependencies.

**Rationale:** Separation enables testability, reuse, and keeps the
library crate dependency-light. If someone wants to embed Souk logic
in their own tooling, they can depend on `souk-core` alone.

### III. Defense in Depth

Souk parses untrusted plugin manifests and executes external processes.
Security is non-negotiable.

- Validate ALL external input at system boundaries: plugin.json,
  marketplace.json, extends-plugin.json, SKILL.md frontmatter.
  Use serde with strict deserialization (deny unknown fields where
  practical).
- NEVER log, print, or embed secrets, tokens, API keys, or credentials
  in output, error messages, or diagnostic data.
- Subprocess execution (Claude CLI) MUST use explicit argument vectors,
  never shell interpolation. Validate and sanitize all arguments passed
  to external processes.
- File operations MUST NOT follow symlinks outside the marketplace
  directory. Path traversal attempts MUST be detected and rejected.
- Principle of least privilege: request only the filesystem access and
  permissions actually needed.

**Rationale:** Plugin manifests come from untrusted sources. A malicious
manifest should never be able to execute code, traverse paths, or leak
data through Souk.

### IV. Fail Fast with Actionable Feedback

Every error MUST help the user fix the problem.

- Error messages MUST include: what failed, where (file path + field if
  applicable), and a suggested fix or next step.
- Example: `"Invalid semver in plugins/foo/1.0/plugin.json field
  'version': '1.0' — expected MAJOR.MINOR.PATCH format (e.g., '1.0.0')"`
- No silent failures. If an operation partially succeeds, report both
  successes and failures explicitly.
- Validation MUST collect all errors in a single pass rather than
  failing on the first error. Users should see every issue at once.
- Use `thiserror` for typed errors with structured context. Avoid
  `.unwrap()` outside of tests.

**Rationale:** A public-facing validation tool that gives cryptic errors
defeats its own purpose. Users should never have to guess what went
wrong or how to fix it.

### V. Test Real Workflows

Test against real directory structures, not mocked abstractions.

- Integration tests MUST use fixture directories that mirror actual
  marketplace structure (marketplace.json, plugin directories, skill
  files).
- Snapshot tests (insta) for CLI output to catch regressions in
  formatting, error messages, and exit codes.
- Mock only what you cannot control: Claude CLI calls go through
  `ClaudeRunner` trait with `MockClaudeRunner` for tests.
- Test every validation rule with both passing and failing inputs.
- Test the add pipeline with all conflict strategies (skip, replace,
  rename) against fixture directories.
- Coverage serves correctness, not metrics. A test that catches real
  bugs is worth more than one that inflates a number.

**Rationale:** Souk replaces battle-tested shell scripts. The test suite
must verify that the Rust implementation matches the behavior users
depend on, not just that abstract units work in isolation.

### VI. Simplicity & YAGNI

Build what's needed now. Refactor when it hurts.

- No speculative abstractions. If a pattern is used once, inline it.
  Generalize at the third repetition (Rule of Three).
- No feature flags, plugin systems, or extensibility hooks unless a
  concrete use case demands them.
- Prefer straightforward code over clever code. If you cannot explain
  a function's purpose in one sentence, split it.
- Configuration should have sensible defaults. The common case should
  require zero flags.
- When in doubt, leave it out. Features can be added; removing them
  breaks users.

**Rationale:** Souk replaces ~2,370 lines of shell scripts. The goal is
a tool that is simpler to maintain, not one that is more complex. Every
abstraction carries a maintenance cost.

### VII. Atomic Safety

All filesystem mutations MUST be atomic with automatic rollback.

- The `AtomicGuard` pattern (RAII with `Drop`) MUST wrap any operation
  that modifies marketplace state (marketplace.json, plugin directories).
- On panic or error, the guard MUST restore the previous state. No
  partial writes, no corrupted marketplace.json.
- The add pipeline follows plan-then-execute: `plan_add()` computes
  what will change, `execute_add()` applies it atomically. Dry-run =
  print the plan without executing.
- Temporary files MUST use `tempfile` crate in the same filesystem to
  enable atomic renames.
- NEVER write directly to marketplace.json. Write to a temp file, then
  atomically rename.

**Rationale:** A corrupted marketplace.json breaks every plugin in the
marketplace. Atomic operations ensure that Souk either fully succeeds
or leaves the marketplace unchanged.

### VIII. Continuous Delivery

Always be in a deployable state. Ship small, ship often.

- The `main` branch MUST always compile, pass tests, and produce
  working binaries.
- Use semantic versioning (SemVer) for releases. Public API changes
  in `souk-core` follow SemVer strictly.
- Prefer small, focused commits with conventional commit messages
  (`type(scope): subject`).
- Cross-platform CI: Linux (x86_64, aarch64), macOS (x86_64, aarch64),
  Windows (x86_64). A release that doesn't build on all targets is not
  a release.
- Releases are automated: tag → CI builds → binaries published.

**Rationale:** As a public-facing CLI tool, users on different platforms
depend on working releases. A broken main branch or a release that only
works on one OS erodes trust.

## Rust Development Standards

These standards apply specifically to Rust code in the Souk workspace.

- **Error handling:** Use `thiserror` for library errors, structured
  error enums in `souk-core`. The CLI crate converts these to
  user-facing output. No `.unwrap()` in library code. `.expect()` only
  with an explanation of the invariant.
- **Serialization:** `serde` with `#[serde(deny_unknown_fields)]` for
  strict manifest parsing where practical. All JSON types MUST
  round-trip: deserialize then serialize produces equivalent output.
- **Dependencies:** Minimize dependency count. Prefer `std` when
  reasonable. Every new dependency MUST be justified by concrete need,
  not convenience.
- **Clippy:** `#[warn(clippy::all)]` at minimum. Address warnings
  before merging — do not suppress without justification.
- **Formatting:** `rustfmt` with default settings. No exceptions.

## Development Workflow

- **Branching:** Trunk-based development. Short-lived feature branches
  merged frequently to `main`.
- **Commits:** Conventional commits (`type(scope): subject`). Scope
  is the crate name or module (e.g., `feat(core): add plugin resolution`).
- **Review:** All changes should be self-reviewed before merge. For
  significant architectural changes, document the decision rationale
  in the commit message or a design note.
- **CI gates:** Compilation, `cargo test`, `cargo clippy`, `cargo fmt
  --check` MUST pass before merge.

## Governance

This constitution governs all development decisions for the Souk
project. When a proposed change conflicts with these principles,
the principles take precedence unless formally amended.

- **Amendments** require updating this document with a version bump,
  rationale for the change, and a sync impact assessment.
- **Versioning** follows SemVer:
  - MAJOR: Removing or fundamentally redefining a principle.
  - MINOR: Adding a principle or materially expanding guidance.
  - PATCH: Clarifications, wording fixes, non-semantic refinements.
- **Compliance:** Use these principles as a checklist during code
  review and architectural decisions. When principles conflict with
  each other (e.g., Simplicity vs. Atomic Safety), the principle
  with higher risk impact takes precedence.

**Version**: 1.0.0 | **Ratified**: 2026-02-26 | **Last Amended**: 2026-02-26
