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
dotdeps add python:requests@2.31.0
dotdeps add node:lodash@4.17.21
dotdeps add node:@org/pkg@1.0.0       # scoped package
dotdeps add go:github.com/org/repo/v2@1.0.0
dotdeps remove python:requests
dotdeps list
dotdeps --clean                       # remove all .deps/
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

## Status

**Work in progress.** Cache and symlink management is complete. The following features are not yet implemented:

- [x] CLI argument parsing
- [x] Cache directory management
- [x] Symlink/copy creation
- [x] List dependencies with broken symlink detection
- [x] Remove dependencies
- [x] Clean command
- [ ] Git cloning with tag resolution
- [ ] Lockfile parsing for automatic version detection
- [ ] Registry repo detection (PyPI, npm, etc.)

## License

MIT
