# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/aaronbassett/souk/releases/tag/souk-core-v0.1.0) - 2026-02-27

### Added

- guard deletes to prevent removing outside pluginRoot
- add shell completions, CI workflows, progress bars, and final integration tests
- add hook and workflow installation for 6 hook managers and 6 CI providers
- add review marketplace command with LLM-powered review
- add add/remove/update commands with atomic operations
- add init command to scaffold new marketplace
- add plugin and marketplace validation with completeness checks
- add marketplace discovery, plugin resolution, and skill resolution
- add error types, serde types for marketplace/plugin/skill, and version constraints

### Fixed

- eliminate env var race in provider detection tests
- make tests cross-platform for Windows CI
- improve error handling in add, update, and remove ops
- add remediation hints to source-of-truth drift error messages
- make update transactional, protect plugin.json with AtomicGuard
- clean up copied dirs on marketplace validation failure in add
- reject symlinks in copy_dir_recursive during plugin add
- use nanos+PID for AtomicGuard backup names, warn on restore failure
- remove duplicate ops module declaration in lib.rs
- apply clippy suggestions across workspace

### Other

- fix formatting in update.rs
- add automated publishing pipeline with release-plz and cargo-dist
- add usage example to remove_plugins doc comment
- fix formatting in main.rs and remove.rs
- fix formatting in AtomicGuard tests
- Merge branch 'worktree-agent-ac886740'
- Merge branch 'worktree-agent-aedf60d5'
