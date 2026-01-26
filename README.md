# dotdeps

CLI tool that fetches dependency source code for LLM context.

## Problem

LLMs hallucinate APIs and rely on stale training data. When documentation is missing or lacking, the source code is the only truth. This tool makes dependency source one file read away.

## Installation

### Pre-built Binaries (Recommended)

Download from [GitHub Releases](https://github.com/davidturnbull/dotdeps/releases/latest).

**macOS/Linux one-liner:**

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/davidturnbull/dotdeps/releases/latest/download/dotdeps-installer.sh | sh
```

### From Source (Rust users)

```bash
cargo install --git https://github.com/davidturnbull/dotdeps
```

## Usage

```bash
dotdeps add <ecosystem>:<package>[@<version>]
dotdeps remove <ecosystem>:<package>
dotdeps list
dotdeps context
dotdeps clean
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
dotdeps clean                            # remove all .deps/
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

## Configuration

Optional config file at `~/.config/dotdeps/config.json`:

```json
{
  "cache_limit_gb": 5,
  "overrides": {
    "python": {
      "some-obscure-lib": {
        "repo": "https://github.com/someone/some-obscure-lib"
      }
    },
    "node": {
      "@private/pkg": {
        "repo": "https://github.com/org/private-pkg"
      }
    }
  }
}
```

### Settings

- `cache_limit_gb`: Maximum cache size in GB (default: 5). Cache is evicted using LRU (least recently used) strategy based on filesystem access time.
- `overrides`: Per-ecosystem, per-package repository URL overrides. Use when automatic detection fails.

Override lookup is case-insensitive for package names.

## License

MIT
