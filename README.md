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
dotdeps add <ecosystem>:<package>@<version>
dotdeps remove <ecosystem>:<package>
dotdeps list
dotdeps --clean
```

### Supported Ecosystems

- `python` - Python packages (PyPI)
- `node` - Node.js packages (npm)
- `go` - Go modules
- `rust` - Rust crates (crates.io)
- `ruby` - Ruby gems (RubyGems)

### Examples

```bash
# Go modules (fully supported)
dotdeps add go:github.com/gin-gonic/gin@1.9.1
dotdeps add go:golang.org/x/sync@0.6.0

# General commands
dotdeps remove go:github.com/gin-gonic/gin
dotdeps list
dotdeps --clean                       # remove all .deps/
```

**Note:** Other ecosystems (python, node, rust, ruby) require registry repo detection, which is not yet implemented.

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

## Status

**Work in progress.** Git cloning is functional for Go modules. Other ecosystems pending registry detection implementation.

- [x] CLI argument parsing
- [x] Cache directory management
- [x] Symlink/copy creation
- [x] List dependencies with broken symlink detection
- [x] Remove dependencies
- [x] Clean command
- [x] Git cloning with tag resolution (shallow clone, --depth 1)
- [x] Go ecosystem: repo URL detection from module path
- [ ] Lockfile parsing for automatic version detection
- [ ] Registry repo detection for Python/Node/Rust/Ruby

## License

MIT
