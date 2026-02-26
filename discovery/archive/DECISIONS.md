# Decision Log: souk-cli

*Chronological record of all decisions made during discovery.*

---

[Decision entries will be added as decisions are made]

## D1: Full scope specification — 2026-02-26

**Context**: Plan has 5 phases. User chose to spec all phases end-to-end.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: Spec all 5 phases: Core Foundation, Plugin Management, Review Commands, CI Commands, Distribution & Polish

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: All

**Related Questions**: [Questions not specified]

---

## D2: Hybrid validation approach — 2026-02-26

**Context**: Existing scripts depend on claude plugin validate. CI needs to run without it. Plan says deterministic only.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: Souk implements all validation rules natively in Rust (deterministic). Optionally shells out to claude plugin validate when available for additional depth. Native checks always run; claude CLI checks are additive.

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: Validate Plugin, Validate Marketplace, CI Validation

**Related Questions**: [Questions not specified]

---

## D3: Add remove and update commands — 2026-02-26

**Context**: Plan covers validate, add, review, ci, init. User wants additional plugin lifecycle commands.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: Add souk remove <plugin> and souk update <plugin> commands to the spec, extending Phase 2 (Plugin Management).

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: Remove Plugin, Update Plugin

**Related Questions**: [Questions not specified]

---

## D4: LLM APIs instead of Claude CLI — 2026-02-26

**Context**: Originally planned hybrid validation with claude plugin validate. User wants no Claude CLI dependency. Instead, use frontier LLM APIs (Anthropic, OpenAI, Gemini) for deep validation/review.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: Replace all claude CLI usage with direct LLM API calls. Support Anthropic, OpenAI, and Gemini APIs. Deterministic validation remains native Rust. AI-powered review uses LLM APIs. D2 (Hybrid validation) is superseded.

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: All review stories, Validate Plugin, Validate Marketplace

**Related Questions**: [Questions not specified]

---

## D5: Remove command: marketplace entry only — 2026-02-26

**Context**: User chose remove should only remove marketplace.json entry by default.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: souk remove removes entry from marketplace.json. --delete flag additionally removes plugin directory from disk. Bumps marketplace version after removal.

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: Remove Plugin

**Related Questions**: [Questions not specified]

---

## D6: Update command: refresh metadata + version bump — 2026-02-26

**Context**: User wants update to re-validate, refresh marketplace metadata, and bump version in plugin.json.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: souk update re-reads plugin.json, updates marketplace.json entry (name/tags/source), re-validates, and bumps version in plugin.json. User can specify --major/--minor/--patch. If not specified, souk analyzes changes and suggests bump type.

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: Update Plugin

**Related Questions**: [Questions not specified]

---

## D7: Validation is pure Rust, LLM for review only — 2026-02-26

**Context**: User confirmed clean separation: validate = deterministic native Rust, review = LLM API calls.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: souk validate performs all checks in pure Rust with no network calls. souk review commands use LLM APIs (Anthropic, OpenAI, Gemini). No --deep or --ai flag on validate.

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: Validate Plugin, Validate Marketplace, Review Plugin, Review Skill, Review Marketplace

**Related Questions**: [Questions not specified]

---

## D8: LLM config via environment variables — 2026-02-26

**Context**: User chose environment variables for API key config.

**Question**: [Question not provided]

**Options Considered**:
[Options not provided]

**Decision**: API keys via ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY. First found is used by default. --provider flag overrides provider selection. --model flag overrides model. No config file for now.

**Rationale**: [Rationale not provided]

**Implications**:
[Implications not provided]

**Stories Affected**: Review Plugin, Review Skill, Review Marketplace

**Related Questions**: [Questions not specified]

---
