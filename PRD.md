# dotdeps

CLI tool that fetches dependency source code for LLM context.

## Problem

LLMs hallucinate APIs and rely on stale training data. When documentation is missing or lacking, the source code is the only truth. This tool makes dependency source one file read away.

## Solution

Manually add dependencies you need to explore. Clone their repos, symlink into `.deps/` for easy LLM browsing.

## Non-goals

- Auto-fetching all dependencies
- Transitive dependencies
- 100% repo detection accuracy
- Automatic sync
- Private registry authentication

## Interface

```
dotdeps add <ecosystem>:<package>[@<version>]
dotdeps remove <ecosystem>:<package>
dotdeps list
dotdeps --clean
```

### Examples

```
dotdeps add python:requests          # version from lockfile
dotdeps add python:requests@2.31.0   # explicit version
dotdeps add node:lodash@4.17.21
dotdeps add node:@org/pkg            # scoped package
dotdeps add go:github.com/org/repo/v2
dotdeps remove python:requests
dotdeps list
dotdeps --clean                      # remove all .deps
```

## Dependency syntax

```
<ecosystem>:<package>@<version>
```

| Component | Required | Notes |
|-----------|----------|-------|
| ecosystem | Yes | python, node, go, rust, ruby |
| package | Yes | Native package name (including scopes, paths) |
| version | No | Inferred from lockfile if omitted |

### Package name formats

| Ecosystem | Format | Example |
|-----------|--------|---------|
| Node.js | name or @scope/name | `lodash`, `@org/pkg` |
| Python | name | `requests` |
| Go | module path | `github.com/org/repo/v2` |
| Rust | name | `serde` |
| Ruby | name | `rails` |

### Case sensitivity

Package names are normalized to lowercase. `Requests` and `requests` resolve to the same cache entry.

## Directory structure

### Cache

```
~/.cache/dotdeps/<ecosystem>/<package>/<version>/
```

Package paths are preserved as nested directories:

```
~/.cache/dotdeps/node/@org/pkg/4.17.21/
~/.cache/dotdeps/go/github.com/org/repo/v2/1.0.0/
```

### Version normalization

Versions are stored without `v` prefix. Tag resolution handles prefix at clone time.

```
Tag: v2.31.0 → Cache: ~/.cache/dotdeps/python/requests/2.31.0/
```

Pre-release versions are stored as-is: `1.0.0-beta.1`

### Project

```
.deps/<ecosystem>/<package> → ~/.cache/dotdeps/<ecosystem>/<package>/<version>
```

Symlinks use absolute paths. Created in current working directory.

### Config

```
~/.config/dotdeps/config.json
```

## Config format

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

## Cache eviction

LRU by filesystem atime. Default cap: 5GB.

## Ecosystems

Priority order for implementation:

| Ecosystem | Lockfile priority | Registry |
|-----------|-------------------|----------|
| Node.js | pnpm-lock.yaml > yarn.lock > package-lock.json | npm |
| Python | poetry.lock > uv.lock > requirements.txt > pyproject.toml | PyPI |
| Go | go.sum | Go module proxy |
| Rust | Cargo.lock | crates.io |
| Ruby | Gemfile.lock | RubyGems |

When multiple lockfiles exist, first match in priority order wins.

## Repo detection

Best-effort. Each registry has metadata fields for repository URL:

- npm: `repository` field
- PyPI: `project_urls`
- crates.io: `repository`
- RubyGems: `source_code_uri`
- Go: module path often is the repo

When detection fails, fall back to global config overrides.

## Git cloning

### Depth

Shallow clone (`--depth 1`) by default. LLMs read current code, not history.

### Tag resolution

Try in order:
1. `v{version}` (e.g., `v2.31.0`)
2. `{version}` (e.g., `2.31.0`)
3. Default branch (warn user)

### Authentication

Uses system git config. SSH keys, credential helpers, and `.netrc` work automatically. No built-in auth handling.

## Special dependency types

### Local path dependencies

Dependencies specified as local paths (`file:../local-pkg`) are skipped silently. Already local; nothing to fetch.

### Git dependencies

Dependencies specified as git URLs (`git+https://...`) in lockfiles:
1. Clone from the git URL directly
2. Use commit hash as version in cache path
3. Checkout the specific commit

```
~/.cache/dotdeps/python/some-git-dep/a1b2c3d4/
```

## Behavior

### dotdeps add

1. Verify cache directory is writable (fail fast if not)
2. Parse ecosystem, package, version from argument
3. Normalize package name to lowercase
4. If version omitted:
   - Find nearest lockfile by walking up from cwd
   - If no lockfile found, error with prompt for explicit version
   - Look up version in lockfile
   - If local path dep, skip silently
   - If git dep, extract URL and commit hash
5. Normalize version (strip `v` prefix if present)
6. Check cache for existing clone
7. If missing:
   - Detect repo URL (or use override, or use git URL for git deps)
   - Shallow clone at matching tag/commit
   - On clone failure, delete partial directory and error
8. Create/overwrite symlink in `.deps/` (cwd)
9. If broken symlinks exist for same package, remove them
10. Evict cache entries if over limit

### dotdeps remove

1. Remove symlink from `.deps/`
2. Do not delete from cache (other projects may use it)
3. Leave `.deps/` directory even if empty

### dotdeps list

1. List current `.deps/` symlinks with versions
2. Warn on any broken symlinks (cache evicted)

### dotdeps --clean

1. Remove `.deps/` directory in cwd

## Failure modes

| Failure | Behavior |
|---------|----------|
| Cache not writable | Error: "Cannot write to ~/.cache/dotdeps. Check permissions." |
| Version not in lockfile | Error: "Version not found. Specify explicitly: dotdeps add python:foo@1.0.0" |
| No lockfile found | Error: "No lockfile found. Specify version explicitly." |
| Lockfile parse error | Error: "Failed to parse {file}:{line}: {details}" |
| Repo URL not found | Error: "Repository not found. Add override to ~/.config/dotdeps/config.json" |
| Tag not found | Warn, fall back to default branch |
| Clone fails | Delete partial directory, error with git message |
| Network error mid-clone | Delete partial directory, error |
| Disk full | Error: "Disk full. Free space or reduce cache_limit_gb." |

## Platform support

### Unix (Linux, macOS)

Symlinks work normally.

### Windows

Symlinks require admin privileges. Detect Windows and copy files instead of symlinking. Document that Windows uses copies (more disk, but functional).

## Concurrency

No explicit protection. Concurrent `dotdeps add` may result in redundant clones but no corruption. Acceptable tradeoff for simplicity.

## File permissions

Inherit from parent directory. No special handling.

## .gitignore

Not managed by dotdeps. User's responsibility to add `.deps/` to `.gitignore` if desired.

## LLM integration

Provide Claude Code skills that teach:

1. When encountering unfamiliar API, run `dotdeps add <ecosystem>:<package>`
2. Browse `.deps/<ecosystem>/<package>` to understand implementation
3. Remove when no longer needed

## Example session

```
$ cd my-project
$ dotdeps add python:requests
Fetching requests 2.31.0... ok
Created .deps/python/requests

$ dotdeps add python:requests@2.32.0
Fetching requests 2.32.0... ok
Updated .deps/python/requests

$ dotdeps add python:obscure-lib
Error: Repository not found for obscure-lib
Add override to ~/.config/dotdeps/config.json

$ dotdeps list
python:requests@2.32.0

$ tree .deps
.deps/
  python/
    requests -> /home/user/.cache/dotdeps/python/requests/2.32.0
```

## Monorepo behavior

- `.deps/` created in cwd (where command is run)
- Version lookup walks up from cwd to find nearest lockfile
- Different subdirectories can have different `.deps/` if desired