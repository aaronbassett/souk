# Feature Specification: souk-cli

**Feature Branch**: `feature/souk-cli`
**Created**: 2026-02-26
**Last Updated**: 2026-02-26
**Status**: In Progress
**Discovery**: See `discovery/` folder for full context

---

## Problem Statement

The midnight-expert project manages a Claude Code plugin marketplace through ~2,370 lines across 13 shell scripts. These scripts handle plugin validation, marketplace management, AI-powered reviews, and CI hooks. They've reached their complexity ceiling: fragile error handling (manual backup/restore), zero test coverage, poor code reuse between scripts, limited extensibility, no cross-platform support, and a hard dependency on external CLI tools for even simple structural checks. Souk replaces all 13 scripts with a single Rust CLI binary (`souk`) backed by a reusable library crate (`souk-core`), adding type safety, comprehensive testing, cross-platform distribution, and direct LLM API integration for AI-powered reviews.

## Personas

| Persona | Description | Primary Goals |
|---------|-------------|---------------|
| Plugin Developer | Creates and maintains Claude Code plugins | Validate plugins locally, get AI-powered review feedback, iterate quickly |
| Marketplace Maintainer | Manages marketplace.json, curates plugin catalog | Add/remove/update plugins safely, ensure consistency, automate validation |
| CI Pipeline | Automated agent in commit/push hooks and CI environments | Fast deterministic checks with no external deps, clear pass/fail exit codes, JSON output |
| New Marketplace Creator | Person starting a new plugin marketplace from scratch | Scaffold directory structure, get to first plugin quickly |

---

## User Scenarios & Testing

<!--
  Stories are ordered by priority (P1 first).
  Each story is independently testable and delivers standalone value.
  Stories may be revised if later discovery reveals gaps - see REVISIONS.md
-->

### Story 1: Validate Plugins [P1] ✅

**As a** Plugin Developer or Marketplace Maintainer,
**I want to** run deterministic validation on one or more plugins,
**So that** I can catch structural errors before committing or publishing.

**CLI surface:**
```
souk validate plugin <path>...
souk validate plugin              # no args = validate all plugins in pluginRoot
```

**Flags:** `--json`, `--quiet`, `--color auto|always|never`, `--marketplace <path>`

#### Acceptance Scenarios

**S1.1: Validate a single plugin by name**
Given a marketplace at `.claude-plugin/marketplace.json` with `pluginRoot: "./plugins"`
When I run `souk validate plugin my-plugin`
Then souk resolves `my-plugin` to `./plugins/my-plugin`
And runs all deterministic checks (see validation rules below)
And prints pass/fail per check with colored output
And exits 0 if all checks pass.

**S1.2: Validate a single plugin by path**
Given a plugin directory at `/tmp/my-plugin/.claude-plugin/plugin.json`
When I run `souk validate plugin /tmp/my-plugin`
Then souk validates the plugin at that exact path
And does not require a marketplace.json to exist.

**S1.3: Validate all plugins (no arguments)**
Given a marketplace with `pluginRoot: "./plugins"` and 5 plugins on disk
When I run `souk validate plugin`
Then souk discovers all immediate subdirectories of `./plugins/`
And validates each one
And prints a summary: `5 plugin(s): 4 passed, 1 failed`
And exits 1 if any plugin fails.

**S1.4: Validate a directory of plugins**
Given a directory `/path/to/plugins/` containing 3 plugin subdirectories
When I run `souk validate plugin /path/to/plugins/`
Then souk detects this is a directory (not a single plugin — no `.claude-plugin/plugin.json`)
And validates all immediate subdirectories that contain `.claude-plugin/plugin.json`.

**S1.5: JSON output mode**
When I run `souk validate plugin my-plugin --json`
Then output is valid JSON:
```json
{
  "results": [
    {"type": "success", "message": "Plugin validated: my-plugin", "details": "path: ./plugins/my-plugin"}
  ]
}
```
And no human-readable text is printed to stdout or stderr.

**S1.6: Quiet mode**
When I run `souk validate plugin my-plugin --quiet`
Then only errors are printed (to stderr)
And success messages are suppressed.

**S1.7: Plugin not found**
When I run `souk validate plugin nonexistent`
Then souk prints `error: Plugin not found: nonexistent`
And exits 1.

**S1.8: No marketplace.json but path provided**
When no `.claude-plugin/marketplace.json` exists
And I run `souk validate plugin ./some-path`
Then souk validates the plugin at that path without requiring marketplace discovery
And marketplace resolution failure is non-fatal.

#### Validation Rules (Deterministic, Native Rust)

These checks run in order. Each produces a diagnostic with severity (error/warning).

| # | Check | Severity | Source |
|---|---|---|---|
| V1.1 | Path exists and is a directory | Error | validate-plugin.sh:87 |
| V1.2 | `.claude-plugin/` directory exists | Error | ci/validate.sh:122 |
| V1.3 | `.claude-plugin/plugin.json` exists | Error | ci/validate.sh:130 |
| V1.4 | `plugin.json` is valid JSON | Error | ci/validate.sh:138 |
| V1.5 | `plugin.json` has `name` field (non-null string) | Error | ci/validate.sh:146 |
| V1.6 | `plugin.json` has `version` field (non-null string) | Error | ci/validate.sh:149 |
| V1.7 | `plugin.json` has `description` field (non-null string) | Error | ci/validate.sh:152 |
| V1.8 | `version` field is valid semver | Error | add-plugin.sh (implied) |
| V1.9 | `extends-plugin.json` (if exists) is valid JSON | Error | validation.sh:29 |
| V1.10 | `extends-plugin.json` has only allowed top-level keys: `dependencies`, `optionalDependencies`, `systemDependencies`, `optionalSystemDependencies` | Error | validation.sh:35-42 |
| V1.11 | Each `extends-plugin.json` section value is an object (not array/primitive) | Error | validation.sh:45-52 |
| V1.12 | Each dependency version constraint matches pattern: `*`, `^x.y.z`, `~x.y.z`, `>=x.y.z`, `<=x.y.z`, `>x.y.z`, `<x.y.z`, `x.y.z`, with optional `-prerelease` suffix | Error | validation.sh:56 |

**Version constraint regex:** `^(\*|[\^~]?[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?|[<>=]+[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?)$`

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E1.1 | Plugin directory is a symlink | Follow symlink, validate target |
| E1.2 | plugin.json has `name: null` | Error: required field missing |
| E1.3 | extends-plugin.json doesn't exist | Skip extends validation (optional file) |
| E1.4 | extends-plugin.json dependency value is object with `version` key | Read `.version` from object, validate that |
| E1.5 | extends-plugin.json dependency value is object without `version` key | Default to `*` |
| E1.6 | extends-plugin.json dependency value is neither string nor object | Error: invalid dependency value |
| E1.7 | Multiple plugins specified, some valid some invalid | Validate all, report each, exit 1 if any failed |
| E1.8 | Plugin root directory doesn't exist when no args given | Error with clear message |
| E1.9 | Empty plugins directory when no args given | Error: no plugins found |

---

### Story 2: Validate Marketplace [P1] ✅

**As a** Marketplace Maintainer or CI Pipeline,
**I want to** validate the marketplace.json and optionally all listed plugins,
**So that** I can ensure the marketplace is structurally sound and complete.

**CLI surface:**
```
souk validate marketplace [--skip-plugins]
```

**Flags:** `--skip-plugins`, `--json`, `--quiet`, `--color auto|always|never`, `--marketplace <path>`

#### Acceptance Scenarios

**S2.1: Full marketplace validation**
When I run `souk validate marketplace`
Then souk validates marketplace.json structure
And cross-checks filesystem vs. marketplace entries (completeness)
And validates every listed plugin (using Story 1 rules)
And prints section-by-section results
And exits 0 if all checks pass.

**S2.2: Skip individual plugin validation**
When I run `souk validate marketplace --skip-plugins`
Then souk validates marketplace.json structure and completeness
But does NOT validate individual plugins
And exits faster.

**S2.3: Completeness check — plugin on disk but not in marketplace**
Given `./plugins/orphan-plugin/` exists on disk
But is not listed in marketplace.json `.plugins[]`
Then souk reports warning: `Plugin in filesystem but not in marketplace: orphan-plugin`
And exits 1.

**S2.4: Completeness check — plugin in marketplace but not on disk**
Given marketplace.json lists `missing-plugin`
But `./plugins/missing-plugin/` does not exist
Then souk reports error: `Plugin in marketplace but not in filesystem: missing-plugin`
And exits 1.

**S2.5: Custom marketplace path**
When I run `souk validate marketplace --marketplace /path/to/marketplace.json`
Then souk uses that file instead of auto-discovering.

**S2.6: JSON output**
When I run `souk validate marketplace --json`
Then output is a single valid JSON object with results array
And each check produces a result entry.

#### Marketplace Validation Rules

| # | Check | Severity | Source |
|---|---|---|---|
| V2.1 | marketplace.json file exists | Error | ci/validate.sh:51 |
| V2.2 | marketplace.json is valid JSON | Error | ci/validate.sh:62 |
| V2.3 | `.plugins` field exists and is an array | Error | ci/validate.sh:67 |
| V2.4 | `.version` field exists and is valid semver | Error | marketplace.json schema |
| V2.5 | `.pluginRoot` (if present) is a string pointing to an existing directory | Error | marketplace.sh:init |
| V2.6 | No duplicate plugin names in `.plugins[]` | Error | add-plugin.sh |
| V2.7 | Each `.plugins[]` entry has `name` (non-empty string) | Error | schema |
| V2.8 | Each `.plugins[]` entry has `source` (non-empty string) | Error | schema |
| V2.9 | Completeness: every directory in pluginRoot has a marketplace entry | Warning | validation.sh:231-244 |
| V2.10 | Completeness: every marketplace entry has a directory in pluginRoot | Error | validation.sh:247-259 |
| V2.11 | (Unless `--skip-plugins`) Each listed plugin passes Story 1 validation | Error | validate-marketplace.sh |

#### Marketplace Discovery

Souk discovers marketplace.json using **upward directory search**:
1. Start from current directory
2. Check for `.claude-plugin/marketplace.json`
3. If not found, move to parent directory
4. Stop at git root (`.git` directory) or filesystem root
5. `--marketplace <path>` flag overrides discovery entirely

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E2.1 | `pluginRoot` not set in marketplace.json | Default to `"./plugins"` |
| E2.2 | `pluginRoot` doesn't start with `./` | Normalize: prepend `./` |
| E2.3 | `pluginRoot` directory doesn't exist | Error with clear message |
| E2.4 | marketplace.json has extra unknown fields | Allow (forward compatibility) |
| E2.5 | `.plugins` is empty array | Valid (marketplace with no plugins yet) |
| E2.6 | Plugin source is absolute path | Accept as-is for resolution |
| E2.7 | Plugin source is relative path starting with `./` or `../` | Resolve relative to marketplace directory |
| E2.8 | Plugin source is bare name | Resolve as `{pluginRoot}/{name}` |
| E2.9 | No marketplace.json found in upward search | Error: marketplace not found |

---

### Story 3: Resolve Plugins and Skills [P1] ✅

**As a** developer of souk-core,
**I want** a robust resolution system for plugins and skills,
**So that** all commands can consistently locate plugins and skills by name, path, or marketplace entry.

This is a library-level story — no direct CLI surface. Used by validate, add, remove, update, and review commands.

#### Plugin Resolution (3-tier)

Given an input string, resolve to an absolute filesystem path:

1. **Direct path**: If input is an existing directory, use it as-is
2. **pluginRoot-relative**: Try `{PLUGIN_ROOT_ABS}/{input}`
3. **Marketplace lookup**: Search `.plugins[].name` for a match, then resolve `.source`

Return error if all three tiers fail.

**Source resolution** (for marketplace `.source` values):
- Starts with `/` → absolute path
- Starts with `./` or `../` → relative to marketplace directory
- Bare name → `{pluginRoot}/{name}`

**Inverse resolution** (`plugin_path_to_source`):
- If path is under pluginRoot, return `{pluginRoot_relative}/{dirname}`
- If path is outside pluginRoot, return absolute path (with non-zero exit code)

#### Skill Resolution (3-tier)

Given a plugin path and skill input string, resolve to a skill directory:

1. **Direct path**: If input is a directory containing `SKILL.md`, use it
2. **Skills subdirectory**: Try `{plugin_path}/skills/{input}/SKILL.md`
3. **Frontmatter name match**: Parse YAML frontmatter from all `{plugin_path}/skills/*/SKILL.md`, match `name:` field against input

**YAML frontmatter parsing**: Extract `name:` from content between `---` delimiters at the start of SKILL.md.

#### Skill Enumeration

List all skills for a plugin:
- Scan `{plugin_path}/skills/*/SKILL.md`
- For each, extract display name from frontmatter `name:` field
- Fall back to directory name if no `name:` in frontmatter

#### Acceptance Scenarios

**S3.1: Resolve plugin by name**
Given pluginRoot is `./plugins` and `./plugins/my-plugin/` exists
When resolving `my-plugin`
Then tier 2 matches and returns absolute path to `./plugins/my-plugin`.

**S3.2: Resolve plugin by path**
When resolving `/absolute/path/to/plugin`
And that directory exists
Then tier 1 matches and returns that path.

**S3.3: Resolve plugin by marketplace name**
Given marketplace.json has `{"name": "Cool Plugin", "source": "cool-plugin"}`
When resolving `Cool Plugin`
Then tier 3 matches via marketplace name, resolves source to `{pluginRoot}/cool-plugin`.

**S3.4: Resolve skill by directory name**
Given plugin at `./plugins/my-plugin` has `skills/commit/SKILL.md`
When resolving skill `commit` for that plugin
Then tier 2 matches.

**S3.5: Resolve skill by frontmatter name**
Given `skills/git-commit/SKILL.md` has frontmatter `name: commit-message`
When resolving skill `commit-message`
Then tier 3 matches via frontmatter.

**S3.6: Enumerate skills**
Given plugin has `skills/a/SKILL.md` (frontmatter name: "Alpha") and `skills/b/SKILL.md` (no name in frontmatter)
When enumerating skills
Then returns `[{dir: "a", display_name: "Alpha"}, {dir: "b", display_name: "b"}]`.

**S3.7: Resolution failure**
When all tiers fail
Then return typed error: `PluginNotFound(input)` or `SkillNotFound(plugin, input)`.

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E3.1 | Plugin source is absolute path outside pluginRoot | Resolve as absolute, inverse resolution returns non-zero |
| E3.2 | Skill frontmatter has no `name:` field | Fall back to directory name |
| E3.3 | Multiple skills have same frontmatter name | First match wins (filesystem order) |
| E3.4 | SKILL.md exists but frontmatter is malformed | Skip frontmatter parsing, use directory name |
| E3.5 | pluginRoot not configured | Default to `./plugins` |
| E3.6 | Empty skills directory | Return empty list for enumeration, error for resolution |

---

### Story 4: Structured Output [P1] ✅

**As a** user (human or CI pipeline),
**I want** consistent output formatting across all commands,
**So that** I can read results visually or parse them programmatically.

This is a cross-cutting library/CLI story. The `Reporter` abstraction is used by all commands.

#### Output Modes

| Mode | Flag | Behavior |
|------|------|----------|
| Human (default) | none | Colored text with sections, checkmarks, errors. Errors to stderr. |
| JSON | `--json` | Single valid JSON object to stdout. No human text. |
| Quiet | `--quiet` | Errors only (to stderr). Success messages suppressed. |

**Color control:** `--color auto|always|never`
- `auto`: color if stdout is a TTY
- `always`: force color (for piping to tools that support ANSI)
- `never`: no color codes

#### Human Mode Colors

| Element | Color | Stream |
|---------|-------|--------|
| Errors | Red, prefixed `ERROR: ` | stderr |
| Warnings | Yellow, prefixed `WARNING: ` | stderr |
| Success | Green, with checkmark prefix | stdout |
| Success (dim) | Dim green, with checkmark prefix | stdout |
| Info | Blue, prefixed `INFO: ` | stdout |
| Section headers | Cyan, wrapped in `=== ... ===` | stdout |

#### JSON Schema

All commands producing JSON use this envelope:
```json
{
  "results": [
    {
      "type": "success" | "error" | "warning" | "info",
      "message": "Human-readable message",
      "details": "Optional additional context"
    }
  ]
}
```

`details` is omitted when empty.

#### Acceptance Scenarios

**S4.1: Human output with colors**
When running any command without `--json` or `--quiet` and stdout is a TTY
Then output uses ANSI colors as defined above.

**S4.2: Human output piped**
When stdout is not a TTY (piped) and `--color` is not set
Then output is plain text with no ANSI escape codes.

**S4.3: JSON output is valid**
When `--json` is passed
Then stdout is a single parseable JSON object matching the schema above.

**S4.4: Quiet mode only shows errors**
When `--quiet` is passed
Then only error messages appear (on stderr)
And exit code reflects success/failure.

**S4.5: JSON escaping**
When a message contains backslashes or double quotes
Then they are properly escaped in JSON output.

**S4.6: Mutually exclusive modes**
If both `--json` and `--quiet` are passed
Then `--json` takes precedence (JSON output, no human text).

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E4.1 | `--color always` but output is piped | Force color codes anyway |
| E4.2 | Empty results (no checks run) | Output `{"results": []}` in JSON mode |
| E4.3 | Very long messages | No truncation — output full message |
| E4.4 | Unicode in messages | Pass through correctly (UTF-8) |

---

### Story 5: Add Plugins to Marketplace [P2]

**As a** Marketplace Maintainer,
**I want to** add one or more plugins to the marketplace with conflict handling and atomic rollback,
**So that** marketplace.json is always left in a consistent state.

**CLI surface:**
```
souk add <path>... [--on-conflict skip|replace|rename] [--dry-run] [--no-copy]
```

**Flags:** `--on-conflict skip|replace|rename` (default: error and abort), `--dry-run`, `--no-copy` (reference external plugins by absolute path instead of copying), `--json`, `--quiet`, `--marketplace <path>`

#### The 7-Phase Pipeline

**Phase 1 — Pre-flight Validation:**
For each plugin argument:
1. Resolve to filesystem path (Story 3)
2. Verify `.claude-plugin/plugin.json` exists
3. Read `name` (required, non-null), `version` (default `"0.1.0"`), `description` (default `""`), `keywords` (default `[]`)
4. Validate plugin using Story 1 rules (unless `--skip-validation` — **removed from spec**: validation should always run)
5. Check for name conflicts in marketplace:
   - **Default (no flag)**: Error and abort
   - **skip**: Remove plugin from processing, warn
   - **replace**: Mark for deletion of old entry first
   - **rename**: Generate unique name by appending `-1`, `-2`, ... until unique

Collect all errors. If any, print all and exit 1.

**Phase 2 — Plan Operations:**
Determine source path for each plugin:
- **Internal** (already under pluginRoot): use relative directory basename
- **External with copy** (default): mark for copy to `{pluginRoot}/{name}/`
- **External with `--no-copy`**: use absolute path

**Phase 3 — Dry Run (if `--dry-run`):**
Print planned operations without executing:
- Which plugins will be added
- Copy operations planned
- Conflict resolutions
- Then exit 0.

**Phase 4 — Copy Operations:**
For external plugins marked for copy:
- Check target directory doesn't already exist (error if it does)
- Copy recursively

**Phase 5 — Atomic Marketplace Update:**
1. Create timestamped backup of marketplace.json
2. For `replace` conflicts: delete old entry from `.plugins[]`
3. Add new entry: `{"name": "...", "source": "...", "tags": [...]}`
4. On ANY failure: restore backup and exit 1

**Phase 6 — Version Bump:**
- Increment minor version of marketplace.json `.version` (e.g., `0.1.0` → `0.2.0`)
- On failure: restore backup

**Phase 7 — Final Validation:**
- Run Story 2 marketplace validation
- On failure: restore backup
- On success: remove backup file

#### Acceptance Scenarios

**S5.1: Add a single internal plugin**
Given `./plugins/new-plugin/` exists and is valid
When I run `souk add ./plugins/new-plugin`
Then new-plugin is added to marketplace.json `.plugins[]`
And marketplace minor version is bumped
And final validation passes.

**S5.2: Add an external plugin (default: copy)**
Given `/tmp/external-plugin/` is a valid plugin
When I run `souk add /tmp/external-plugin`
Then the plugin directory is copied to `{pluginRoot}/external-plugin/`
And a marketplace entry is added with source relative to pluginRoot.

**S5.3: Add external plugin with --no-copy**
When I run `souk add /tmp/external-plugin --no-copy`
Then no files are copied
And marketplace entry uses absolute path as source.

**S5.4: Dry run**
When I run `souk add new-plugin --dry-run`
Then operations are printed but not executed
And marketplace.json is unchanged.

**S5.5: Name conflict — default (abort)**
When a plugin named `existing` already exists in marketplace
And I run `souk add ./plugins/existing`
Then souk reports error and exits 1 without modifying marketplace.

**S5.6: Name conflict — skip**
When I run `souk add plugin-a plugin-b --on-conflict skip`
And `plugin-a` already exists
Then `plugin-a` is skipped with a warning
And `plugin-b` is added normally.

**S5.7: Name conflict — replace**
When I run `souk add ./plugins/existing --on-conflict replace`
Then the old marketplace entry for `existing` is removed
And the new entry is added.

**S5.8: Name conflict — rename**
When I run `souk add ./plugins/existing --on-conflict rename`
Then the plugin is added as `existing-1` (or `-2`, etc.)
And the name is unique in the marketplace.

**S5.9: Rollback on failure**
When Phase 5 or 6 fails (e.g., invalid JSON after modification)
Then the backup is restored
And marketplace.json is unchanged from before the command ran.

**S5.10: Multiple plugins**
When I run `souk add plugin-a plugin-b plugin-c`
Then all three are added in a single atomic operation
And a single version bump occurs.

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E5.1 | All plugins skipped due to conflicts | Exit 0 with warning, no marketplace changes |
| E5.2 | Target copy directory already exists | Error for that plugin, continue with others if `--on-conflict skip` |
| E5.3 | Plugin has no `name` in plugin.json | Error: required field |
| E5.4 | Plugin has `name: null` | Error: required field |
| E5.5 | Version field missing during add | Default to `"0.1.0"` |
| E5.6 | Unique name generation loops | Generate `-1`, `-2`, ... with no upper bound |
| E5.7 | Marketplace.json doesn't exist yet | Error: use `souk init` first |

---

### Story 6: Remove Plugins from Marketplace [P2]

**As a** Marketplace Maintainer,
**I want to** remove plugins from the marketplace,
**So that** I can deprecate or delist plugins cleanly.

**CLI surface:**
```
souk remove <name>... [--delete]
```

**Flags:** `--delete` (also remove plugin directory from disk), `--json`, `--quiet`, `--marketplace <path>`

#### Acceptance Scenarios

**S6.1: Remove by name**
When I run `souk remove my-plugin`
Then the entry with `name: "my-plugin"` is removed from marketplace.json `.plugins[]`
And the marketplace minor version is bumped
And the plugin directory remains on disk.

**S6.2: Remove with --delete**
When I run `souk remove my-plugin --delete`
Then the marketplace entry is removed
And the plugin directory is deleted from disk
And the marketplace version is bumped.

**S6.3: Remove multiple plugins**
When I run `souk remove plugin-a plugin-b`
Then both entries are removed atomically
And a single version bump occurs.

**S6.4: Plugin not in marketplace**
When I run `souk remove nonexistent`
Then souk reports error: `Plugin not found in marketplace: nonexistent`
And exits 1 without modifying marketplace.

**S6.5: Atomic rollback**
When removal succeeds but version bump fails
Then the backup is restored.

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E6.1 | `--delete` but plugin directory doesn't exist on disk | Remove marketplace entry, warn about missing directory |
| E6.2 | Plugin source is absolute path with `--delete` | Delete at that absolute path |
| E6.3 | Last plugin removed | Valid: marketplace has empty `.plugins[]` array |

---

### Story 7: Update Plugins in Marketplace [P2]

**As a** Marketplace Maintainer,
**I want to** refresh a plugin's metadata and bump its version,
**So that** marketplace.json reflects the current state of plugin files and versions are tracked.

**CLI surface:**
```
souk update <name>... [--major|--minor|--patch]
```

**Flags:** `--major`, `--minor`, `--patch` (version bump type), `--json`, `--quiet`, `--marketplace <path>`

#### Acceptance Scenarios

**S7.1: Update metadata with explicit version bump**
Given `my-plugin` is in the marketplace and its plugin.json has changed on disk
When I run `souk update my-plugin --minor`
Then souk re-reads `plugin.json` from disk
And updates the marketplace entry (name, tags from keywords, source)
And bumps the `version` field in `plugin.json` from e.g. `1.2.3` → `1.3.0`
And re-validates the plugin (Story 1 rules)
And bumps marketplace version.

**S7.2: Update with automatic bump suggestion**
When I run `souk update my-plugin` (no bump flag)
Then souk analyzes changes to the plugin (e.g., compares current vs. marketplace metadata)
And suggests a bump type (major/minor/patch)
And prompts the user for confirmation
And applies the chosen bump.

**S7.3: Update multiple plugins**
When I run `souk update plugin-a plugin-b --patch`
Then both plugins are updated and bumped.

**S7.4: Plugin not in marketplace**
When I run `souk update nonexistent`
Then souk reports error and exits 1.

**S7.5: Validation failure after update**
When the updated plugin fails validation
Then the update is rolled back (original plugin.json and marketplace.json restored).

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E7.1 | Plugin.json has no version field | Initialize to `0.1.0` before bumping |
| E7.2 | Plugin files deleted from disk | Error: plugin directory not found |
| E7.3 | Automatic bump with no detectable changes | Suggest patch bump, inform user |

---

### Story 8: Scaffold New Marketplace [P2]

**As a** New Marketplace Creator,
**I want to** scaffold a new marketplace directory structure,
**So that** I can start managing plugins immediately.

**CLI surface:**
```
souk init [--path <dir>]
```

#### Acceptance Scenarios

**S8.1: Init in current directory**
When I run `souk init`
Then souk creates:
```
.claude-plugin/
  marketplace.json    # {"version": "0.1.0", "pluginRoot": "./plugins", "plugins": []}
plugins/              # empty directory
```
And prints success message.

**S8.2: Init at specified path**
When I run `souk init --path /tmp/my-marketplace`
Then the structure is created at that path.

**S8.3: Directory already has marketplace**
When `.claude-plugin/marketplace.json` already exists
Then souk reports error: `Marketplace already exists at .claude-plugin/marketplace.json`
And exits 1 without modifying anything.

**S8.4: Init with custom pluginRoot**
When I run `souk init --plugin-root ./extensions`
Then `marketplace.json` has `"pluginRoot": "./extensions"`
And an `extensions/` directory is created.

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E8.1 | Parent directory doesn't exist | Create it recursively |
| E8.2 | Permission denied | Error with clear message |

---

### Story 9: AI-Review Plugin [P3]

**As a** Plugin Developer,
**I want to** get an AI-powered review of my plugin,
**So that** I get expert feedback on quality, security, and best practices.

**CLI surface:**
```
souk review plugin <name|path> [--output-dir <path>] [--provider <name>] [--model <name>]
```

**Flags:** `--output-dir` (default: `./reviews/{plugin-name}/`), `--provider anthropic|openai|gemini`, `--model <model-id>`, `--json`, `--quiet`

#### LLM Provider Configuration (D4, D7, D8)

API keys via environment variables:
- `ANTHROPIC_API_KEY` → Anthropic API
- `OPENAI_API_KEY` → OpenAI API
- `GEMINI_API_KEY` → Gemini API

Provider selection:
1. `--provider` flag if specified
2. First available key found (in order: Anthropic, OpenAI, Gemini)
3. Error if no key found

Model selection:
1. `--model` flag if specified
2. Default per provider (configurable sensible defaults)

#### Acceptance Scenarios

**S9.1: Review a plugin**
Given `ANTHROPIC_API_KEY` is set
When I run `souk review plugin my-plugin`
Then souk resolves the plugin (Story 3)
And reads all plugin files
And sends a structured review prompt to the Anthropic API
And saves the review as `./reviews/my-plugin/review-report.md`
And prints a summary to stdout.

**S9.2: Specify provider and model**
When I run `souk review plugin my-plugin --provider openai --model gpt-4o`
Then souk uses the OpenAI API with gpt-4o.

**S9.3: Custom output directory**
When I run `souk review plugin my-plugin --output-dir /tmp/reviews`
Then the report is saved to `/tmp/reviews/review-report.md`.

**S9.4: JSON output with metadata**
When I run `souk review plugin my-plugin --json`
Then output includes:
```json
{
  "plugin": "my-plugin",
  "path": "./plugins/my-plugin",
  "review_date": "2026-02-26T12:00:00Z",
  "provider": "anthropic",
  "model": "claude-sonnet-4-20250514",
  "report_file": "./reviews/my-plugin/review-report.md"
}
```

**S9.5: No API key available**
When no API key environment variables are set
Then souk reports error: `No LLM API key found. Set one of: ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY`
And exits 1.

#### Review Prompt Structure

The review prompt requests analysis of:
1. Executive Summary
2. Component Analysis (agents, skills, commands, hooks, MCP servers)
3. Code Quality Assessment
4. Documentation Review
5. Security Considerations
6. Recommendations

(Exact prompt templates to be defined during implementation)

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E9.1 | API rate limit | Retry with exponential backoff (max 3 retries) |
| E9.2 | API returns error | Report error with API message, exit 1 |
| E9.3 | Very large plugin (exceeds context window) | Truncate or summarize before sending |
| E9.4 | Output directory already has a report | Overwrite with new report |
| E9.5 | Network timeout | Error with suggestion to retry |

---

### Story 10: AI-Review Skills [P3]

**As a** Plugin Developer,
**I want to** get an AI-powered review of specific skills within a plugin,
**So that** I can improve individual skill quality.

**CLI surface:**
```
souk review skill <plugin> [skill[,skill,...]] [--all] [--output-dir <path>]
```

**Flags:** `--all` (review all skills), `--output-dir`, `--provider`, `--model`, `--json`, `--quiet`

#### Acceptance Scenarios

**S10.1: Review a specific skill by name**
When I run `souk review skill my-plugin commit-message`
Then souk resolves the plugin and skill (Story 3)
And sends skill content to LLM API
And saves report to `./reviews/my-plugin/commit-message/review-report.md`.

**S10.2: Review multiple skills (comma-separated)**
When I run `souk review skill my-plugin "skill-a,skill-b"`
Then both skills are reviewed
And reports are saved in separate subdirectories.

**S10.3: Interactive selection (no skill argument)**
When I run `souk review skill my-plugin`
Then souk lists available skills with numbered menu
And allows selecting one, multiple (comma/range), or "all"
And reviews selected skills.

**S10.4: Review all skills**
When I run `souk review skill my-plugin --all`
Then all skills in the plugin are reviewed.

**S10.5: Skill not found**
When I run `souk review skill my-plugin nonexistent`
Then souk reports error and exits 1.

#### Interactive Selection

Menu format:
```
Skills in my-plugin:
  1. commit-message
  2. code-review
  3. pr-template
  a. All skills

Select skill(s) to review [1-3, a]:
```

Selection parsing:
- Single number: `1`
- Comma-separated: `1,3`
- Range: `1-3`
- Mixed: `1,3-5`
- All: `a` or `all`

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E10.1 | Plugin has no skills | Error: no skills found |
| E10.2 | Invalid selection (out of range) | Re-prompt with error message |
| E10.3 | Some skills fail review, others succeed | Report each result, exit 1 if any failed |
| E10.4 | Non-interactive mode (piped input) with no skill arg | Error: skill argument required when not interactive |

---

### Story 11: AI-Review Marketplace [P3]

**As a** Marketplace Maintainer,
**I want to** get an AI-powered review of the entire marketplace,
**So that** I can assess overall quality, consistency, and completeness.

**CLI surface:**
```
souk review marketplace [--output-dir <path>]
```

**Flags:** `--output-dir` (default: `./reviews/marketplace/`), `--provider`, `--model`, `--json`, `--quiet`

#### Acceptance Scenarios

**S11.1: Review entire marketplace**
When I run `souk review marketplace`
Then souk reads marketplace.json and all plugin manifests
And sends structured prompt to LLM API requesting:
  - Overall marketplace assessment
  - Plugin consistency analysis
  - Coverage gaps
  - Quality distribution
  - Recommendations
And saves report to `./reviews/marketplace/review-report.md`.

**S11.2: JSON output**
When I run `souk review marketplace --json`
Then metadata includes marketplace path, plugin count, provider used, and report path.

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E11.1 | Empty marketplace (no plugins) | Review structure only, note emptiness |
| E11.2 | Very large marketplace | Summarize plugin list before sending to LLM |

---

### Story 12: Run Pre-Commit Validation [P4]

**As a** CI Pipeline (git pre-commit hook),
**I want to** validate only the plugins that have staged changes,
**So that** commits are fast and only check what changed.

**CLI surface:**
```
souk ci run pre-commit
```

#### Acceptance Scenarios

**S12.1: Changed plugins validated**
Given git has staged changes in `plugins/my-plugin/some-file.txt`
When the pre-commit hook runs `souk ci run pre-commit`
Then souk detects `my-plugin` from staged file paths
And validates only `my-plugin` (Story 1 rules)
And exits 0 if valid.

**S12.2: Marketplace.json staged**
Given `marketplace.json` is staged
When pre-commit runs
Then souk additionally validates marketplace structure (Story 2 with `--skip-plugins`).

**S12.3: No staged files**
When no files are staged
Then souk exits 0 immediately.

**S12.4: Multiple changed plugins**
Given changes in `plugins/a/` and `plugins/b/`
Then both plugins are validated
And exit 1 if either fails.

#### Plugin Detection

Extract plugin names from staged file paths:
- Get staged files via `git diff --cached --name-only`
- Match pattern: `{pluginRoot_relative}/{name}/...`
- Deduplicate names

**Note:** Unlike the current `ci/commit.sh` which hardcodes `plugins/`, souk must use the configured `pluginRoot` from marketplace.json.

#### Edge Cases

| ID | Scenario | Handling |
|----|----------|----------|
| E12.1 | Staged file not under pluginRoot | Ignored (not a plugin change) |
| E12.2 | Plugin directory deleted in staged changes | Skip validation for that plugin |
| E12.3 | No marketplace.json found | Error: marketplace not found |
| E12.4 | pluginRoot pattern doesn't match any staged files | Exit 0 (no plugin changes) |

---

### Story 13: Run Pre-Push Validation [P4]

**As a** CI Pipeline (git pre-push hook),
**I want to** run full marketplace validation before pushing,
**So that** only valid marketplaces are pushed to remote.

**CLI surface:**
```
souk ci run pre-push
```

#### Acceptance Scenarios

**S13.1: Full validation**
When the pre-push hook runs `souk ci run pre-push`
Then souk runs full `validate marketplace` (Story 2, including all plugins)
And exits 0 if all pass, 1 if any fail.

**S13.2: Failure message**
When validation fails
Then souk prints error summary
And suggests `git push --no-verify` to bypass.

---

### Story 14: Install Git Hooks [P4]

**As a** Marketplace Maintainer,
**I want to** install git hooks that run souk validation,
**So that** validation happens automatically on commit and push.

**CLI surface:**
```
souk ci install hooks [--native|--lefthook|--husky|--overcommit|--hk|--simple-git-hooks]
```

#### Hook Manager Detection

If no flag specified, auto-detect:
1. Check for config files: `lefthook.yml`, `.husky/`, `.overcommit.yml`, `hk.toml`, `.simple-git-hooks.json`
2. Check global installs: `which lefthook`, `which husky`, etc.
3. Default: native git hooks (`.git/hooks/`)
4. Confirm choice with user via interactive prompt

#### Acceptance Scenarios

**S14.1: Install native git hooks**
When I run `souk ci install hooks --native`
Then souk creates `.git/hooks/pre-commit` and `.git/hooks/pre-push`
With content that calls `souk ci run pre-commit` / `souk ci run pre-push`
And marks them executable.

**S14.2: Install lefthook config**
When I run `souk ci install hooks --lefthook`
Then souk adds/updates `lefthook.yml` with pre-commit and pre-push stanzas.

**S14.3: Auto-detect with confirmation**
Given `lefthook.yml` exists in the project
When I run `souk ci install hooks`
Then souk detects lefthook and prompts: `Detected lefthook. Install hooks via lefthook? [Y/n]`

**S14.4: Already installed**
When hooks are already configured
Then souk reports what exists and asks to overwrite.

#### Supported Managers

| Manager | Config Detection | Generated Config |
|---------|-----------------|------------------|
| Native git | (default) | `.git/hooks/pre-commit`, `.git/hooks/pre-push` |
| Lefthook | `lefthook.yml` | YAML snippet |
| Husky | `.husky/` | Shell scripts in `.husky/` |
| Overcommit | `.overcommit.yml` | YAML snippet |
| hk | `hk.toml` | TOML snippet |
| simple-git-hooks | `.simple-git-hooks.json` | JSON entry |

---

### Story 15: Install CI Workflows [P4]

**As a** Marketplace Maintainer,
**I want to** generate CI workflow files for my CI provider,
**So that** marketplace validation runs automatically on PRs.

**CLI surface:**
```
souk ci install workflows [--github|--blacksmith|--northflank|--circleci|--gitlab|--buildkite]
```

#### CI Provider Detection

If no flag specified, auto-detect:
1. Check for config directories: `.github/workflows/`, `.circleci/`, `.gitlab-ci.yml`, `.buildkite/`
2. Check CLI tools: `blacksmith`, `glab`, `bk`
3. Check git remote URL for provider hints
4. Confirm choice with user

#### Acceptance Scenarios

**S15.1: Generate GitHub Actions workflow**
When I run `souk ci install workflows --github`
Then souk creates `.github/workflows/souk-validate.yml`
With a workflow that installs souk and runs `souk validate marketplace`.

**S15.2: Auto-detect with confirmation**
Given `.github/workflows/` exists
When I run `souk ci install workflows`
Then souk detects GitHub Actions and confirms.

**S15.3: Workflow already exists**
When the workflow file already exists
Then souk asks to overwrite or skip.

#### Supported Providers

| Provider | Detection | Generated File |
|----------|-----------|----------------|
| GitHub Actions | `.github/workflows/` | `.github/workflows/souk-validate.yml` |
| Blacksmith | `blacksmith` CLI | `.github/workflows/souk-validate.yml` (Blacksmith-flavored) |
| NorthFlank | northflank config | Northflank pipeline config |
| CircleCI | `.circleci/` | `.circleci/config.yml` (partial) |
| GitLab CI | `.gitlab-ci.yml` | `.gitlab-ci.yml` (partial) |
| Buildkite | `.buildkite/` | `.buildkite/pipeline.yml` (partial) |

---

### Story 16: Distribution & Shell Completions [P5]

**As a** user,
**I want** souk distributed as pre-built binaries with shell completions,
**So that** I can install it easily on any platform.

#### Acceptance Scenarios

**S16.1: Cross-platform binaries**
CI builds and publishes binaries for:
- Linux x86_64 and aarch64
- macOS x86_64 and aarch64
- Windows x86_64

**S16.2: Shell completions**
When I run `souk completions bash|zsh|fish`
Then souk outputs shell completion script for the specified shell to stdout.

**S16.3: Man pages**
The build generates man pages via `clap_mangen`.

**S16.4: Progress bars**
Batch operations (validating many plugins, reviewing multiple skills) show progress bars via `indicatif`.

**S16.5: Rich error display**
Errors include:
- File paths pointing to the problem
- Field names for JSON validation errors
- Suggestions for how to fix

---

## Edge Cases

| ID | Scenario | Handling | Stories Affected |
|----|----------|----------|------------------|
| E1.1 | Plugin directory is a symlink | Follow symlink, validate target | 1, 5 |
| E1.2 | plugin.json has `name: null` | Error: required field missing | 1, 5 |
| E2.1 | pluginRoot not set in marketplace.json | Default to `"./plugins"` | 2, 3, 5, 12, 13 |
| E2.8 | Plugin source is bare name | Resolve as `{pluginRoot}/{name}` | 2, 3, 5 |
| E5.9 | Rollback on atomic update failure | Restore backup, report error | 5, 6, 7 |
| E9.1 | API rate limit | Retry with exponential backoff (max 3) | 9, 10, 11 |
| E12.1 | Staged file not under pluginRoot | Ignored | 12 |

---

## Requirements

### Functional Requirements

| ID | Requirement | Stories | Confidence |
|----|-------------|---------|------------|
| FR1 | All validation rules run in pure Rust with no external process calls | 1, 2 | 100% |
| FR2 | AI review commands use LLM APIs (Anthropic, OpenAI, Gemini) — no Claude CLI | 9, 10, 11 | 100% |
| FR3 | API keys configured via environment variables | 9, 10, 11 | 100% |
| FR4 | All marketplace mutations use atomic backup/restore pattern | 5, 6, 7 | 100% |
| FR5 | Plugin/skill resolution follows 3-tier hierarchy | 3 | 100% |
| FR6 | All commands support `--json`, `--quiet`, `--color` flags | 4 | 100% |
| FR7 | souk-core library crate has no CLI/output concerns | All | 100% |
| FR8 | Upward directory search for marketplace.json, stoppable with `--marketplace` | 2, 5, 6, 7, 12, 13 | 100% |
| FR9 | Version constraint regex matches: `*`, `^x.y.z`, `~x.y.z`, `>=x.y.z`, `<=x.y.z`, `>x.y.z`, `<x.y.z`, `x.y.z`, with optional prerelease | 1 | 100% |
| FR10 | `souk remove` removes marketplace entry only by default; `--delete` also removes files | 6 | 100% |
| FR11 | `souk update` refreshes metadata and bumps version in plugin.json; bump type via flags or auto-suggest | 7 | 100% |
| FR12 | Pre-commit hook uses pluginRoot config (not hardcoded `plugins/`) to detect changed plugins | 12 | 100% |

### Key Entities

**marketplace.json schema:**
```json
{
  "version": "semver string",
  "pluginRoot": "./plugins (optional, defaults to ./plugins)",
  "plugins": [
    {
      "name": "string (required)",
      "source": "string (required — bare name, relative path, or absolute path)",
      "tags": ["string array (optional)"]
    }
  ]
}
```

**plugin.json schema:**
```json
{
  "name": "string (required, non-null)",
  "version": "semver string (required, non-null)",
  "description": "string (required, non-null)",
  "keywords": ["string array (optional)"]
}
```

**extends-plugin.json schema:**
```json
{
  "dependencies": { "name": "version-constraint" },
  "optionalDependencies": { "name": "version-constraint or {version: constraint}" },
  "systemDependencies": { "name": "version-constraint" },
  "optionalSystemDependencies": { "name": "version-constraint" }
}
```
Only these four top-level keys are allowed. Each value must be an object. Dependency values are strings or objects with a `version` key.

---

## Success Criteria

| ID | Criterion | Measurement | Stories |
|----|-----------|-------------|---------|
| SC1 | Feature parity with all 13 shell scripts | Every validation rule, add phase, review prompt, CI hook ported | All |
| SC2 | Zero external runtime dependencies for validation | `souk validate` works with no other tools installed | 1, 2 |
| SC3 | Cross-platform builds | CI matrix produces binaries for 5 targets (Linux/macOS/Windows × architectures) | 16 |
| SC4 | Comprehensive test coverage | Unit tests for every validation rule, integration tests for every command | All |
| SC5 | Snapshot tests for CLI output | insta snapshots for human and JSON output of all commands | 4 |
| SC6 | Parity check passes | Running souk validate against the actual midnight-expert marketplace produces same results as existing scripts | 1, 2 |
| SC7 | Sub-second validation | `souk validate marketplace` completes in under 1 second for a marketplace with 20 plugins | 1, 2, 16 |

---

## Appendix: Story Revision History

*Major revisions to graduated stories. Full details in `archive/REVISIONS.md`*

| Date | Story | Change | Reason |
|------|-------|--------|--------|
| *No revisions yet* | - | - | - |
