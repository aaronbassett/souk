# Execution Prompt

Paste the following into a new Claude Code session at the repo root:

---

```
Execute the implementation plan at docs/plans/2026-02-26-souk-cli.md using parallel agents where possible.

The spec is at discovery/SPEC.md. Reference scripts being replaced are in temp-reference-scripts/.

## Execution Strategy

Use worktree-isolated agents for parallel work. Execute in waves — each wave completes before the next starts. Within each wave, dispatch all agents simultaneously.

### Wave 1: Foundation (sequential — single agent)
- Task 1: Create Cargo workspace skeleton
- This must complete first since every other task depends on it.

### Wave 2: Core Library Types (2 parallel agents)
- **Agent A** (worktree): Tasks 2 + 3 — Error types + Serde types (souk-core/src/error.rs, souk-core/src/types/)
- **Agent B** (worktree): Task 9 — Reporter output abstraction (souk/src/output.rs)
- These touch completely different crates and have no file overlap.

### Wave 3: Resolution + Validation (2 parallel agents)
- **Agent A** (worktree): Tasks 4 + 5 + 6 — Marketplace discovery + Plugin resolution + Skill resolution (souk-core/src/discovery.rs, souk-core/src/resolution/)
- **Agent B** (worktree): Tasks 7 + 8 — Plugin validation + Marketplace validation (souk-core/src/validation/)
- Both depend on Wave 2's types. No file overlap between resolution and validation modules.

### Wave 4: CLI Integration (sequential — single agent)
- Task 10: Build CLI with clap + validate commands (souk/src/cli.rs, souk/src/commands/, souk/src/main.rs)
- Task 11: Test fixtures + integration tests (tests/)
- These wire everything together — must see the full codebase.

### Wave 5: Plugin Management (2 parallel agents)
- **Agent A** (worktree): Tasks 12 + 13 — AtomicGuard + Version bumping (souk-core/src/ops/atomic.rs, souk-core/src/version.rs)
- **Agent B** (worktree): Task 14 — Init command (souk-core/src/ops/init.rs, souk/src/commands/init.rs)
- No file overlap.

### Wave 6: Add/Remove/Update (sequential — single agent)
- Task 15: Add command (7-phase pipeline)
- Task 16: Remove command
- Task 17: Update command
- These share AtomicGuard, version bumping, and marketplace mutation patterns. Sequential is safer.

### Wave 7: Review Commands (2 parallel agents)
- **Agent A** (worktree): Task 18 — LLM provider abstraction (souk-core/src/review/provider.rs, mod.rs)
- **Agent B**: Waits for Agent A, then Tasks 19 + 20 + 21 run — but 19/20/21 can be parallelized:
  - **Agent B1** (worktree): Task 19 — Review plugin
  - **Agent B2** (worktree): Task 20 — Review skill
  - **Agent B3** (worktree): Task 21 — Review marketplace

### Wave 8: CI Commands (2 parallel agents)
- **Agent A** (worktree): Tasks 22 + 23 — Pre-commit + Pre-push hooks (souk-core/src/ci/hooks.rs)
- **Agent B** (worktree): Tasks 24 + 25 — Hook installation + Workflow installation (souk-core/src/ci/install_hooks.rs, install_workflows.rs)

### Wave 9: Distribution (sequential)
- Tasks 26–29: Shell completions, progress bars, CI workflows, final integration tests

## Rules for Each Agent

1. Read the full task from the plan before starting
2. Follow TDD: write failing test → implement → verify pass → commit
3. Each agent commits its own work with descriptive messages
4. Do NOT modify files outside your assigned scope
5. Run `cargo test` for your crate before committing
6. If a task in the plan has complete code, use it as-is. If it says "expand before execution", read the spec story and reference scripts to fill in the implementation details.

## After Each Wave

1. Merge all worktree branches into main working branch
2. Run `cargo test --workspace` to verify no conflicts
3. Run `cargo clippy --workspace` for lint check
4. Fix any merge conflicts or compilation errors before proceeding to next wave
5. Commit the merge

## Key References

- **Full spec**: discovery/SPEC.md (16 stories with acceptance scenarios, validation rules, edge cases)
- **Architecture**: PLAN.md (repo structure, dependency table, command tree)
- **Reference scripts**: temp-reference-scripts/ (the 13 bash scripts being replaced)
- **Decisions**: discovery/archive/DECISIONS.md (8 key design decisions, especially D4: LLM APIs not Claude CLI, D7: validation=pure Rust)
```
