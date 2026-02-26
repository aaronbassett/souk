# Souk

A CLI tool for managing Claude Code plugin marketplaces. Handles validation, plugin lifecycle, AI-powered reviews, and CI integration.

## Features

- **Validate** plugins and marketplaces with deterministic, structured diagnostics
- **Add, remove, and update** plugins with atomic operations and automatic rollback
- **AI-powered reviews** of plugins, skills, and entire marketplaces via Anthropic, OpenAI, or Gemini
- **CI integration** with git hooks and workflow generation for 6 CI providers
- **Machine-readable output** with `--json` for scripting and automation
- **Cross-platform** binaries for Linux, macOS, and Windows

## Installation

### From source

```bash
cargo install --path crates/souk
```

### From GitHub releases

Download the latest binary for your platform from [Releases](https://github.com/aaronbassett/souk/releases).

### Shell completions

```bash
souk completions bash > ~/.local/share/bash-completion/completions/souk
souk completions zsh > ~/.zfunc/_souk
souk completions fish > ~/.config/fish/completions/souk.fish
```

## Usage

```
souk [OPTIONS] <COMMAND>
```

### Global options

| Flag | Description |
|------|-------------|
| `--json` | Output machine-readable JSON |
| `--quiet` | Suppress non-error output |
| `--color <auto\|always\|never>` | Color mode |
| `--marketplace <path>` | Override marketplace.json auto-discovery |

### Initialize a marketplace

```bash
souk init
souk init --path ./my-project --plugin-root ./extensions
```

### Validate

```bash
# Validate specific plugins
souk validate plugin ./plugins/my-plugin ./plugins/other-plugin

# Validate the entire marketplace
souk validate marketplace

# Skip per-plugin checks
souk validate marketplace --skip-plugins

# Machine-readable output
souk validate marketplace --json
```

### Add plugins

```bash
# Add a plugin (copies to pluginRoot)
souk add ./path/to/plugin

# Add without copying
souk add ./external/plugin --no-copy

# Handle name conflicts
souk add ./plugin --on-conflict replace  # or: skip, rename, abort (default)

# Preview without executing
souk add ./plugin --dry-run
```

### Remove plugins

```bash
souk remove "My Plugin"

# Also delete plugin directory from disk
souk remove "My Plugin" --delete
```

### Update plugins

```bash
# Refresh metadata from disk
souk update "My Plugin"

# Bump version
souk update "My Plugin" --patch   # 1.0.0 -> 1.0.1
souk update "My Plugin" --minor   # 1.0.0 -> 1.1.0
souk update "My Plugin" --major   # 1.0.0 -> 2.0.0
```

### AI-powered reviews

Requires an API key in `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or `GEMINI_API_KEY`.

```bash
souk review plugin my-plugin
souk review skill my-plugin --all
souk review marketplace
```

### CI integration

```bash
# Run validation hooks
souk ci run pre-commit   # validates changed plugins
souk ci run pre-push     # full marketplace validation

# Install git hooks (auto-detects hook manager)
souk ci install hooks                  # native git hooks
souk ci install hooks --lefthook       # lefthook
souk ci install hooks --husky          # husky

# Install CI workflows (auto-detects provider)
souk ci install workflows              # auto-detect
souk ci install workflows --github     # GitHub Actions
souk ci install workflows --gitlab     # GitLab CI
```

## Architecture

Cargo workspace with two crates:

```
crates/
  souk-core/    # Library: domain logic, no CLI concerns
  souk/         # Binary: clap CLI, output formatting
```

**`souk-core`** exposes all functionality as a library so other tools can depend on it directly.

## Development

```bash
# Build
cargo build --workspace

# Test
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --check
```

## License

MIT

