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
| `go`      | -                    | Module path |
| `node`    | -                    | -          |
| `rust`    | -                    | -          |
| `ruby`    | -                    | -          |

### Examples

```bash
# Python (fully supported)
dotdeps add python:requests              # version from lockfile
dotdeps add python:requests@2.31.0       # explicit version
dotdeps add python:flask
dotdeps add python:typing-extensions

# Go modules (fully supported)
dotdeps add go:github.com/gin-gonic/gin@1.9.1
dotdeps add go:golang.org/x/sync@0.6.0

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

## Status

**Work in progress.** Python and Go ecosystems are fully functional. Other ecosystems pending implementation.

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
- [ ] Node ecosystem: lockfile parsing and npm registry detection
- [ ] Rust ecosystem: Cargo.lock parsing and crates.io detection
- [ ] Ruby ecosystem: Gemfile.lock parsing and RubyGems detection

## License

MIT
