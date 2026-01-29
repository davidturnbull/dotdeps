# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2](https://github.com/davidturnbull/dotdeps/compare/v0.1.1...v0.1.2) - 2026-01-29

### Fixed

- *(npm)* handle ssh:// protocol in repository URLs

## [0.1.1](https://github.com/davidturnbull/dotdeps/compare/v0.1.0...v0.1.1) - 2026-01-29

### Added

- *(update)* add self-update command for automatic updates
- *(init)* add init command for project setup

### Fixed

- *(ci)* use git_only mode for release-plz

### Other

- *(readme)* document update command usage
- *(context)* improve instruction text clarity and formatting
- release v0.1.0
- update README
- configure release-plz to skip crates.io publish
- skip e2e tests that require external tools

## [0.1.0](https://github.com/davidturnbull/dotdeps/releases/tag/v0.1.0) - 2026-01-26

### Added

- *(init)* add init command for project setup
- add --dry-run flag for previewing actions without changes
- add --json flag for machine-readable output
- support bun.lock files for Node.js ecosystem
- handle git dependencies and local path deps in lockfiles
- implement Swift ecosystem Package.resolved parsing and repo detection
- support monorepo tag formats ({package}-{version}, {package}-v{version})
- implement LRU cache eviction based on filesystem atime
- implement config file support for repo URL overrides
- implement Go ecosystem go.sum parsing and version lookup
- implement Ruby ecosystem Gemfile.lock parsing and RubyGems repo detection
- implement Rust ecosystem Cargo.lock parsing and crates.io repo detection
- implement Node.js ecosystem lockfile parsing and npm repo detection
- implement Python ecosystem lockfile parsing and PyPI repo detection
- implement git cloning with tag resolution
- implement cache and .deps directory management
- implement CLI argument parsing with clap

### Other

- update README
- configure release-plz to skip crates.io publish
- skip e2e tests that require external tools
- add release workflows and cargo-dist config
- remove Status section from README
- Add end-to-end test harness
- *(cli)* simplify dry-run output formatting
- *(cli)* change clean from flag to subcommand
- Add context command
- changes
- Add final project summary - 100% Homebrew replication achieved
- Update commands list to include all 123 implemented commands
- Implement final 8 Ruby DSL delegation commands - 100% coverage achieved
- Implement 36 Ruby DSL delegation commands (batch 3)
- Implement 11 Ruby DSL delegation commands (batch 2)
- Implement Ruby DSL delegation commands (5 commands)
- Implement audit command (cmd-032)
- Implement postinstall command (cmd-015)
- Fix update-report command (cmd-027)
- Implement update-if-needed command (cmd-026)
- Implement edit command (cmd-046)
- Implement docs command (cmd-045)
- Implement update-reset command (cmd-028)
- Implement nodenv-sync, pyenv-sync, rbenv-sync commands (cmd-014, cmd-016, cmd-017)
- Implement tab command (cmd-023)
- Implement source command (cmd-022)
- Implement tap-info command (cmd-024)
- Implement which-formula command (cmd-031)
- Implement formula command (cmd-049)
- Implement unalias command (cmd-025)
- Implement missing command (cmd-013)
- Implement completions command (cmd-008)
- Implement command-not-found-init command (cmd-006)
- Implement command command (cmd-007)
- Implement formulae command (cmd-010)
- Implement casks command (cmd-005)
- Implement shellenv command (cmd-021)
- Implement analytics command (cmd-003)
- Implement alias command (cmd-002)
- Implement --env command (cmd-001)
- Create comprehensive task list for Homebrew replication
- empty progress
- Optimize uses command with reverse dependency map (cmd-027)
- Implement options command (cmd-034)
- Implement log command (cmd-032)
- Implement home command (cmd-031)
- Implement cat command (cmd-033)
- Implement desc command (cmd-030)
- Implement autoremove command (cmd-029)
- Implement leaves command (cmd-028)
- Implement uses command (cmd-027) - incomplete due to performance
- Implement reinstall command (cmd-026)
- Implement doctor command (cmd-025)
- Implement untap command (cmd-022)
- Implement tap command (cmd-021)
- Implement cleanup command (cmd-020)
- Implement pin and unpin commands (cmd-023, cmd-024)
- Implement upgrade command (cmd-018)
- Implement update command (cmd-017)
- Implement outdated command (cmd-019)
- Implement unlink command (cmd-016)
- Refactor link command with let-chains pattern
- Implement link command (cmd-015)
- Implement uninstall command (cmd-014)
- Implement deps command (cmd-013)
- Implement install command (cmd-012)
- Implement dependency resolution (arch-007)
- Implement installation infrastructure (arch-006)
- Update prompt instructions for task completion criteria
- Implement download and caching infrastructure (arch-005)
- Implement search command (cmd-011)
- Implement info command (cmd-010)
- Implement list command (cmd-009)
- Implement tap management (arch-004)
- Implement formula parsing and representation (arch-003)
- Implement config command
- Implement --include-aliases for commands command
- Implement help command with command-specific help
- Mark cmd-005 (--repository with tap arguments) as passing
- Implement --cache with formula/cask arguments
- Implement --cellar with formula arguments and extract formula module
- Implement full --prefix command with formula, --installed, --unbrewed
- Implement core CLI framework and path detection
- changes
- Merge commit '1a9845bb5f2f1712cc5fb3c824e6013e4c591815' as 'vendor/brew'
- Squashed 'vendor/brew/' content from commit 4bcf90a01d
