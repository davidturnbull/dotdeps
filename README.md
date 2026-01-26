# dotdeps

## Introduction

LLM coding agents have outdated training data and can't read documentation in real-time. When they help you use a library, they work from memory, which leads to hallucinated APIs and incorrect usage patterns.

dotdeps solves this by cloning dependency source code to `.deps/` where Claude Code can read it. Repositories are cached globally and symlinked into projects, so fetching is fast after the first clone. After setup, Claude fetches the source for any dependency it's unsure about.

## Quick start

1. Install:

   ```bash
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/davidturnbull/dotdeps/releases/latest/download/dotdeps-installer.sh | sh
   ```

2. Add to `~/.bashrc` or `~/.zshrc`:

   ```bash
   alias claude='command claude --append-system-prompt "$(dotdeps context)"'
   ```

That's it. Claude Code now automatically fetches dependency source when needed.

## Supported ecosystems

| Ecosystem | Lockfiles                                              | Repo detection |
| --------- | ------------------------------------------------------ | -------------- |
| `python`  | poetry.lock, uv.lock, requirements.txt, pyproject.toml | PyPI API       |
| `node`    | pnpm-lock.yaml, yarn.lock, package-lock.json, bun.lock | npm registry   |
| `rust`    | Cargo.lock                                             | crates.io API  |
| `go`      | go.sum                                                 | Module path    |
| `ruby`    | Gemfile.lock                                           | RubyGems API   |
| `swift`   | Package.resolved                                       | Lockfile URL   |

### Python

When no version is specified, dotdeps searches for lockfiles (walking up from the current directory) in this priority order:

1. `poetry.lock` - Poetry lockfile
2. `uv.lock` - uv lockfile
3. `requirements.txt` - pip requirements (only exact pins: `==`)
4. `pyproject.toml` - Poetry or PEP 621 dependencies

Repository URLs are detected via the PyPI API.

### Node.js

When no version is specified, dotdeps searches for lockfiles in this priority order:

1. `pnpm-lock.yaml` - pnpm lockfile
2. `yarn.lock` - Yarn classic and Berry lockfiles
3. `package-lock.json` - npm lockfile (v1, v2, and v3)
4. `bun.lock` - Bun lockfile

Repository URLs are detected via the npm registry. Scoped packages (e.g., `@types/node`) are fully supported.

### Rust

When no version is specified, dotdeps searches for `Cargo.lock` walking up from the current directory.

Repository URLs are detected via the crates.io API. Crate names are normalized for comparison (case-insensitive, `-` and `_` treated as equivalent).

### Go

When no version is specified, dotdeps searches for `go.sum` walking up from the current directory.

Go module paths (e.g., `github.com/gin-gonic/gin`) are used directly as repository URLs. Module paths with major version suffixes (e.g., `/v2`, `/v3`) are handled correctly.

### Ruby

When no version is specified, dotdeps searches for `Gemfile.lock` walking up from the current directory.

Repository URLs are detected via the RubyGems API. Gem names are case-insensitive. Platform-specific version suffixes (e.g., `-x86_64-linux`, `-arm64-darwin`, `-java`) are stripped automatically.

### Swift

When no version is specified, dotdeps searches for `Package.resolved` in these locations:

1. `Package.resolved` - Swift Package Manager
2. `*.xcodeproj/project.xcworkspace/xcshareddata/swiftpm/Package.resolved` - Xcode project
3. `*.xcworkspace/xcshareddata/swiftpm/Package.resolved` - Xcode workspace

Both v1 (Swift < 5.6) and v2 (Swift 5.6+) lockfile formats are supported. Repository URLs are read directly from the lockfile. Aliases: `swift`, `swiftpm`, `spm`.

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
    }
  }
}
```

### cache_limit_gb

Maximum cache size in GB. Default: 5.

When to adjust: increase for large projects with many dependencies, or set to 0 for unlimited.

How it works: cache is evicted using LRU (least recently used) strategy based on filesystem access time.

### overrides

Per-ecosystem, per-package repository URL overrides.

When to use: private packages, obscure libraries without proper metadata, or forks you want to use instead of the original.

Lookup is case-insensitive for package names.

## Manual usage

For manual dependency management (without Claude Code integration):

```bash
dotdeps add <ecosystem>:<package>[@<version>]
dotdeps remove <ecosystem>:<package>
dotdeps list
dotdeps context
dotdeps clean
```

### Examples

```bash
# Python
dotdeps add python:requests              # version from lockfile
dotdeps add python:requests@2.31.0       # explicit version
dotdeps add python:flask
dotdeps add python:typing-extensions

# Node.js
dotdeps add node:lodash                  # version from lockfile
dotdeps add node:lodash@4.17.21          # explicit version
dotdeps add node:express@4.18.0
dotdeps add node:@types/node             # scoped packages

# Rust
dotdeps add rust:serde                   # version from Cargo.lock
dotdeps add rust:serde@1.0.228           # explicit version
dotdeps add cargo:clap@4.5.0             # cargo alias

# Go
dotdeps add go:github.com/gin-gonic/gin    # version from go.sum
dotdeps add go:github.com/gin-gonic/gin@1.9.1
dotdeps add go:golang.org/x/sync@0.6.0

# Ruby
dotdeps add ruby:rails                   # version from Gemfile.lock
dotdeps add ruby:rails@7.1.0             # explicit version
dotdeps add ruby:sidekiq
dotdeps add ruby:nokogiri

# Swift
dotdeps add swift:swift-argument-parser  # version from Package.resolved
dotdeps add swift:Alamofire@5.9.0        # explicit version
dotdeps add spm:swift-nio                # spm alias

# General commands
dotdeps remove python:requests
dotdeps list
dotdeps clean                            # remove all .deps/
```

## Behind the scenes

1. `dotdeps add` resolves the version (explicit or from lockfile)
2. Checks cache at `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`
3. If not cached, clones the repository (shallow clone with tag resolution)
4. Creates symlink at `.deps/<ecosystem>/<package>`
5. LRU cache eviction when limit exceeded

## Related projects

- [opensrc](https://github.com/vercel-labs/opensrc) - Fetch source for npm packages
- [better-context](https://github.com/davis7dotsh/better-context) - Query library source with AI

## License

MIT
