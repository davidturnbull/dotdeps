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

- `python` - Python packages (PyPI)
- `node` - Node.js packages (npm)
- `go` - Go modules
- `rust` - Rust crates (crates.io)
- `ruby` - Ruby gems (RubyGems)

### Examples

```bash
dotdeps add python:requests          # version from lockfile
dotdeps add python:requests@2.31.0   # explicit version
dotdeps add node:lodash@4.17.21
dotdeps add node:@org/pkg            # scoped package
dotdeps add go:github.com/org/repo/v2
dotdeps remove python:requests
dotdeps list
dotdeps --clean                      # remove all .deps/
```

## Status

**Work in progress.** CLI argument parsing is complete. The following features are not yet implemented:

- [ ] Cache directory management
- [ ] Git cloning with tag resolution
- [ ] Lockfile parsing (Python, Node, Go, Rust, Ruby)
- [ ] Registry repo detection
- [ ] Symlink/copy creation
- [ ] List and remove commands
- [ ] Clean command

## License

MIT
