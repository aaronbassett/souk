# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/aaronbassett/souk/releases/tag/souk-v0.1.0) - 2026-02-27

### Added

- guard deletes to prevent removing outside pluginRoot
- add shell completions, CI workflows, progress bars, and final integration tests
- add hook and workflow installation for 6 hook managers and 6 CI providers
- add add/remove/update commands with atomic operations
- add init command to scaffold new marketplace
- add CLI with clap, validate commands, test fixtures, and integration tests
- create cargo workspace skeleton with souk-core and souk crates
- add Reporter output abstraction (human/json/quiet modes)

### Fixed

- improve error handling in add, update, and remove ops

### Other

- add automated publishing pipeline with release-plz and cargo-dist
- fix formatting in main.rs and remove.rs
- Merge branch 'worktree-agent-ac886740'
