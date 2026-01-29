# dotdeps

CLI tool that fetches dependency source code for LLM context.

## Why

LLM coding agents have outdated training data and can't read documentation in real-time. When they help you use a library, they work from memory, which leads to hallucinated APIs and incorrect usage patterns.

dotdeps solves this by cloning dependency source code to `.deps/` where your agent can read it. Repositories are cached globally and symlinked into projects, so fetching is fast after the first clone.

## Quick start

1. Install:

   ```bash
   curl --proto '=https' --tlsv1.2 -LsSf https://github.com/davidturnbull/dotdeps/releases/latest/download/dotdeps-installer.sh | sh
   ```

2. Initialize in your project:

   ```bash
   dotdeps init
   ```

This creates `.deps/`, updates `.gitignore`, and adds usage instructions to `AGENTS.md` (or `CLAUDE.md` if it exists).

## Keeping up to date

Run `dotdeps update` periodically to get the latest version:

```bash
dotdeps update
```

Use `dotdeps update --check` to check for updates without installing.

## Commands

### init

Initialize dotdeps in the current directory.

```bash
dotdeps init [OPTIONS]
```

#### Options

- `--skip-gitignore` - Skip adding `.deps/` to `.gitignore`
- `--skip-instructions` - Skip adding usage instructions to `AGENTS.md`/`CLAUDE.md`
- `--dry-run` - Preview actions without making changes
- `--json` - Output results as JSON

### add

Add a dependency to `.deps/`.

```bash
dotdeps add [OPTIONS] <ecosystem>:<package>[@<version>]
```

#### Arguments

- `<ecosystem>` - Package ecosystem: `python`, `node`, `rust`, `go`, `ruby`, `swift`
- `<package>` - Package name
- `@<version>` - Optional version (defaults to lockfile version)

#### Options

- `--dry-run` - Preview actions without making changes
- `--json` - Output results as JSON

#### Examples

```bash
dotdeps add python:requests           # version from lockfile
dotdeps add python:requests@2.31.0    # explicit version
dotdeps add node:lodash
dotdeps add node:@types/node          # scoped packages
dotdeps add rust:serde
dotdeps add go:github.com/gin-gonic/gin
dotdeps add ruby:rails
dotdeps add swift:Alamofire
```

### remove

Remove a dependency from `.deps/`.

```bash
dotdeps remove [OPTIONS] <ecosystem>:<package>
```

#### Options

- `--dry-run` - Preview actions without making changes
- `--json` - Output results as JSON

#### Example

```bash
dotdeps remove python:requests
```

### list

List all dependencies in `.deps/`.

```bash
dotdeps list [OPTIONS]
```

#### Options

- `--json` - Output results as JSON

### context

Output LLM-ready dependency context. This prints instructions that tell your agent which dependencies are available and how to fetch more.

```bash
dotdeps context [OPTIONS]
```

#### Options

- `--json` - Output results as JSON

#### Lockfile discovery

`dotdeps context` searches upward from the current directory to find lockfiles. In monorepos or nested project layouts, this means dependencies from a parent directory's lockfile may be included. Run from the specific project directory you want to analyze.

### clean

Remove the `.deps/` directory.

```bash
dotdeps clean [OPTIONS]
```

#### Options

- `--dry-run` - Preview actions without making changes
- `--json` - Output results as JSON

## Supported ecosystems

| Ecosystem | Lockfiles                                              | Repo detection |
| --------- | ------------------------------------------------------ | -------------- |
| `python`  | poetry.lock, uv.lock, requirements.txt, pyproject.toml | PyPI API       |
| `node`    | pnpm-lock.yaml, yarn.lock, package-lock.json, bun.lock | npm registry   |
| `rust`    | Cargo.lock                                             | crates.io API  |
| `go`      | go.sum                                                 | Module path    |
| `ruby`    | Gemfile.lock                                           | RubyGems API   |
| `swift`   | Package.resolved                                       | Lockfile URL   |

## Configuration

Optional config file at `~/.config/dotdeps/config.json`:

```json
{
  "cache_limit_gb": 5,
  "overrides": {
    "python": {
      "some-private-lib": {
        "repo": "https://github.com/myorg/some-private-lib"
      }
    }
  }
}
```

### cache_limit_gb

Maximum cache size in GB. Default: `5`. Set to `0` for unlimited.

Cache eviction uses LRU (least recently used) based on filesystem access time.

### overrides

Per-ecosystem, per-package repository URL overrides.

Use for: private packages, libraries without proper metadata, or forks.

## Claude Code integration

For automatic context injection with Claude Code, add this alias to `~/.bashrc` or `~/.zshrc`:

```bash
alias claude='command claude --append-system-prompt "$(dotdeps context)"'
```

This injects dependency context into every Claude Code session, so Claude automatically knows which dependencies are available and how to fetch more.

## How it works

1. `dotdeps add` resolves the version (explicit or from lockfile)
2. Checks cache at `~/.cache/dotdeps/<ecosystem>/<package>/<version>/`
3. If not cached, clones the repository (shallow clone with tag resolution)
4. Creates symlink at `.deps/<ecosystem>/<package>/`
5. LRU cache eviction when limit exceeded

## Related projects

- [opensrc](https://github.com/vercel-labs/opensrc) - Fetch source for npm packages
- [better-context](https://github.com/davis7dotsh/better-context) - Query library source with AI

## License

MIT
