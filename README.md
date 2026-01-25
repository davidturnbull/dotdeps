# dotdeps

CLI tool that fetches dependency source code for LLM context.

## Problem

LLMs hallucinate APIs and rely on stale training data. When documentation is missing or lacking, the source code is the only truth. This tool makes dependency source one file read away.

## Installation

```bash
cargo install --path .
```

## Usage

```bash
dotdeps add <ecosystem>:<package>[@<version>]
dotdeps remove <ecosystem>:<package>
dotdeps list
dotdeps --clean
```

### Supported Ecosystems

| Ecosystem | Version from Lockfile | Repo Detection |
|-----------|----------------------|----------------|
| `python`  | poetry.lock, uv.lock, requirements.txt, pyproject.toml | PyPI API |
| `node`    | pnpm-lock.yaml, yarn.lock, package-lock.json | npm registry |
| `rust`    | Cargo.lock           | crates.io API  |
| `go`      | go.sum               | Module path |
| `ruby`    | Gemfile.lock         | RubyGems API   |

### Examples

```bash
# Python (fully supported)
dotdeps add python:requests              # version from lockfile
dotdeps add python:requests@2.31.0       # explicit version
dotdeps add python:flask
dotdeps add python:typing-extensions

# Node.js (fully supported)
dotdeps add node:lodash                  # version from lockfile
dotdeps add node:lodash@4.17.21          # explicit version
dotdeps add node:express@4.18.0
dotdeps add node:@types/node             # scoped packages

# Rust (fully supported)
dotdeps add rust:serde                   # version from Cargo.lock
dotdeps add rust:serde@1.0.228           # explicit version
dotdeps add cargo:clap@4.5.0             # cargo alias

# Go modules (fully supported)
dotdeps add go:github.com/gin-gonic/gin    # version from go.sum
dotdeps add go:github.com/gin-gonic/gin@1.9.1
dotdeps add go:golang.org/x/sync@0.6.0

# Ruby (fully supported)
dotdeps add ruby:rails                   # version from Gemfile.lock
dotdeps add ruby:rails@7.1.0             # explicit version
dotdeps add ruby:sidekiq
dotdeps add ruby:nokogiri

# General commands
dotdeps remove python:requests
dotdeps list
dotdeps --clean                          # remove all .deps/
```

## Directory Structure

### Cache

Dependencies are cached at:

```
~/.cache/dotdeps/<ecosystem>/<package>/<version>/
```

### Project

Symlinks are created at:

```
.deps/<ecosystem>/<package> -> ~/.cache/dotdeps/<ecosystem>/<package>/<version>
```

## Python Lockfile Support

When no version is specified, dotdeps searches for lockfiles (walking up from the current directory) in this priority order:

1. `poetry.lock` - Poetry lockfile
2. `uv.lock` - uv lockfile
3. `requirements.txt` - pip requirements (only exact pins: `==`)
4. `pyproject.toml` - Poetry or PEP 621 dependencies

## Node.js Lockfile Support

When no version is specified, dotdeps searches for lockfiles in this priority order:

1. `pnpm-lock.yaml` - pnpm lockfile
2. `yarn.lock` - Yarn classic and Berry lockfiles
3. `package-lock.json` - npm lockfile (v1, v2, and v3)

Scoped packages (e.g., `@types/node`) are fully supported.

## Rust Lockfile Support

When no version is specified, dotdeps searches for `Cargo.lock` walking up from the current directory.

Crate names are normalized for comparison (case-insensitive, `-` and `_` treated as equivalent).

## Ruby Lockfile Support

When no version is specified, dotdeps searches for `Gemfile.lock` walking up from the current directory.

Gem names are case-insensitive. Platform-specific version suffixes (e.g., `-x86_64-linux`, `-arm64-darwin`, `-java`) are stripped automatically.

## Go Module Support

When no version is specified, dotdeps searches for `go.sum` walking up from the current directory.

Go module paths (e.g., `github.com/gin-gonic/gin`) are used directly as repository URLs. Module paths with major version suffixes (e.g., `/v2`, `/v3`) are handled correctly.

## Status

**Work in progress.** Python, Node.js, Rust, Go, and Ruby ecosystems are fully functional. Other features pending implementation.

- [x] CLI argument parsing
- [x] Cache directory management
- [x] Symlink/copy creation
- [x] List dependencies with broken symlink detection
- [x] Remove dependencies
- [x] Clean command
- [x] Git cloning with tag resolution (shallow clone, --depth 1)
- [x] Go ecosystem: repo URL detection from module path
- [x] Python ecosystem: lockfile parsing (poetry.lock, uv.lock, requirements.txt, pyproject.toml)
- [x] Python ecosystem: PyPI repo URL detection
- [x] Node ecosystem: lockfile parsing (pnpm-lock.yaml, yarn.lock, package-lock.json)
- [x] Node ecosystem: npm registry repo URL detection
- [x] Rust ecosystem: Cargo.lock parsing
- [x] Rust ecosystem: crates.io repo URL detection
- [x] Ruby ecosystem: Gemfile.lock parsing
- [x] Ruby ecosystem: RubyGems repo URL detection
- [x] Go ecosystem: go.sum lockfile parsing
- [ ] Config file support (cache limits, repo overrides)

## License

MIT
