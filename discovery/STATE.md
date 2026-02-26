# Discovery State: souk-cli

**Updated**: 2026-02-26 12:50 UTC
**Iteration**: 3
**Phase**: Story Development (all stories written, seeking graduation confirmation)

---

## Problem Understanding

### Problem Statement
The midnight-expert project manages a Claude Code plugin marketplace through ~2,370 lines across 13 shell scripts. These scripts handle plugin validation, marketplace management, AI-powered reviews, and CI hooks. They've reached their complexity ceiling: fragile error handling (manual backup/restore), zero test coverage, poor code reuse between scripts, limited extensibility, no cross-platform support, and a hard dependency on `claude` CLI even for simple structural checks. Souk replaces all scripts with a Rust CLI binary distributed from a new repository, adding type safety, comprehensive testing, cross-platform builds, and direct LLM API integration for AI-powered reviews.

### Personas
| Persona | Description | Primary Goals |
|---------|-------------|---------------|
| Plugin Developer | Creates and maintains Claude Code plugins | Validate plugins locally, get AI-powered review feedback, iterate quickly |
| Marketplace Maintainer | Manages marketplace.json, curates plugin catalog | Add/remove/update plugins safely, ensure consistency, automate validation |
| CI Pipeline | Automated validation in commit/push hooks and CI | Fast deterministic checks without external deps, clear pass/fail signals |
| New Marketplace Creator | Wants to start a new plugin marketplace from scratch | Scaffold directory structure, get running quickly |

### Current State vs. Desired State
**Today**: Plugin developers run bash scripts that depend on `claude` CLI and `jq`. Error handling is fragile. No tests. No Windows support. Adding features means editing interconnected bash scripts.

**Tomorrow**: A single `souk` binary with proper error types, RAII rollback, comprehensive tests, cross-platform distribution, and direct LLM API integration for reviews.

### Constraints
- Must achieve feature parity with all 13 existing scripts
- Library crate (souk-core) must have no CLI/output concerns
- Deterministic validation must work with no external dependencies
- Must support JSON, human-readable, and quiet output modes
- Cross-platform: Linux, macOS, Windows (x86_64 + aarch64 where applicable)
- LLM reviews use API calls (Anthropic, OpenAI, Gemini) — no Claude CLI

---

## Story Landscape

### Story Status Overview
| # | Story | Priority | Status | Confidence | Blocked By |
|---|-------|----------|--------|------------|------------|
| 1 | Validate plugins | P1 | ✅ In SPEC | 100% | - |
| 2 | Validate marketplace | P1 | ✅ In SPEC | 100% | - |
| 3 | Resolve plugins and skills | P1 | ✅ In SPEC | 100% | - |
| 4 | Structured output (JSON/human/quiet) | P1 | ✅ In SPEC | 100% | - |
| 5 | Add plugins to marketplace | P2 | ✅ In SPEC | 100% | 1, 2 |
| 6 | Remove plugins from marketplace | P2 | ✅ In SPEC | 100% | 2 |
| 7 | Update plugins in marketplace | P2 | ✅ In SPEC | 100% | 5, 6 |
| 8 | Scaffold new marketplace | P2 | ✅ In SPEC | 100% | - |
| 9 | AI-review plugin | P3 | ✅ In SPEC | 100% | 3 |
| 10 | AI-review skills | P3 | ✅ In SPEC | 100% | 3, 9 |
| 11 | AI-review marketplace | P3 | ✅ In SPEC | 100% | 9 |
| 12 | Run pre-commit validation | P4 | ✅ In SPEC | 100% | 1, 2 |
| 13 | Run pre-push validation | P4 | ✅ In SPEC | 100% | 2 |
| 14 | Install git hooks | P4 | ✅ In SPEC | 100% | 12, 13 |
| 15 | Install CI workflows | P4 | ✅ In SPEC | 100% | - |
| 16 | Distribution & shell completions | P5 | ✅ In SPEC | 100% | All |

### Story Dependencies
```
[1: Validate Plugin] ──┬──> [5: Add Plugin] ──> [7: Update Plugin]
                       │                            ↑
[2: Validate Mktpl] ──┤──> [6: Remove Plugin] ─────┘
                       │
[3: Resolve P/S] ──────┤──> [9: Review Plugin] ──> [10: Review Skill]
                       │                       └──> [11: Review Mktpl]
[4: Output Modes] ─────┤
                       │
                       ├──> [12: Pre-commit] ──┬──> [14: Install Hooks]
                       └──> [13: Pre-push] ────┘
                                               └──> [15: Install Workflows]

[8: Init/Scaffold] (independent)
[16: Distribution] (depends on all)
```

### Proto-Stories / Emerging Themes
*All proto-stories crystallized and graduated to SPEC.md.*

---

## Completed Stories Summary

| # | Story | Priority | Completed | Key Decisions | Revision Risk |
|---|-------|----------|-----------|---------------|---------------|
| 1 | Validate plugins | P1 | 2026-02-26 | Pure Rust, 12 validation rules | Low |
| 2 | Validate marketplace | P1 | 2026-02-26 | Upward discovery, completeness check | Low |
| 3 | Resolve plugins/skills | P1 | 2026-02-26 | 3-tier resolution for both | Low |
| 4 | Structured output | P1 | 2026-02-26 | JSON/human/quiet, color control | Low |
| 5 | Add plugins | P2 | 2026-02-26 | 7-phase pipeline, 4 conflict strategies | Low |
| 6 | Remove plugins | P2 | 2026-02-26 | Entry only by default, --delete flag | Low |
| 7 | Update plugins | P2 | 2026-02-26 | Refresh metadata + version bump | Medium (auto-suggest logic TBD) |
| 8 | Scaffold marketplace | P2 | 2026-02-26 | souk init with --plugin-root option | Low |
| 9 | Review plugin | P3 | 2026-02-26 | LLM APIs, 3 providers | Medium (prompt templates TBD) |
| 10 | Review skills | P3 | 2026-02-26 | Interactive selection, batch mode | Medium |
| 11 | Review marketplace | P3 | 2026-02-26 | Whole-marketplace review | Medium |
| 12 | Pre-commit | P4 | 2026-02-26 | Uses pluginRoot (not hardcoded) | Low |
| 13 | Pre-push | P4 | 2026-02-26 | Full validation | Low |
| 14 | Install hooks | P4 | 2026-02-26 | 6 hook managers | Low |
| 15 | Install workflows | P4 | 2026-02-26 | 6 CI providers | Low |
| 16 | Distribution | P5 | 2026-02-26 | 5 build targets | Low |

*Full stories in SPEC.md*

---

## In-Progress Story Detail

[No stories in progress — all graduated]

---

## Watching List

*Items that might affect graduated stories:*
- Story 7: Auto-suggest version bump logic needs further definition during implementation
- Stories 9-11: Review prompt templates need to be designed during implementation
- Story 14: Hook manager config formats may need research during implementation

---

## Glossary

- **Marketplace**: A curated collection of Claude Code plugins defined by a `marketplace.json` file
- **Plugin**: A Claude Code extension with a `.claude-plugin/plugin.json` manifest
- **Skill**: A capability within a plugin, defined by a `SKILL.md` file with YAML frontmatter
- **pluginRoot**: The directory (relative to project root) where plugin directories live, configured in marketplace.json
- **extends-plugin.json**: Optional file declaring plugin dependencies with semver constraints
- **Source**: The path reference to a plugin in marketplace.json (can be relative basename, explicit relative, or absolute)
- **Deterministic validation**: Structural/schema checks that require no external tools beyond the binary itself
- **LLM API**: Direct HTTP API calls to Anthropic, OpenAI, or Gemini for AI-powered reviews

---

## Next Actions

- Get user confirmation that spec is complete
- Clear OPEN_QUESTIONS.md if no remaining questions
