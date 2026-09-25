# Changelog

## 1.2.2

- **MCP server** now runs on [mcp-kit](https://github.com/SylphxAI/mcp-kit), which uses rmcp, the official Rust MCP SDK, instead of repomap's own JSON-RPC loop. Tools, tool names and answers are unchanged. The server now also handles protocol negotiation across every spec version, cancellation, progress and pagination.
- **Shared parts:** `setup`, the npm launcher and the release workflow now come from mcp-kit, shared with the other Sylphx MCP servers.

## 1.2.1

- **Security (`serve`, `db --serve`):**
  - Query and `file://` URI percent-decoding now works on bytes. Malformed input such as `?q=%aé` could panic a request thread.
  - Requests are served by a fixed pool of 8 workers instead of one thread per request.
  - A non-loopback `--host` now always requires a token. Pass `--token` or `REPOMAP_TOKEN`, or repomap generates one and prints it in the URL. The token is accepted as `?token=`, a Bearer header, or an HttpOnly cookie, and compared in constant time.
- **Release:** npm packages publish with trusted publishing (OIDC, with provenance) instead of a long-lived token.
- **Copy:**
  - The README tool table is generated from the tool registry (`repomap tools`). There are six tools, including `db`.
  - One one-liner (`brand.json`) feeds the README, docs, npm, the MCP Registry and the GitHub description, and CI checks that they match.
  - Migration notes now live in one place.
- **Score:** `repomap score --badge-style static` writes a self-hosted `.github/agent-ready.svg` for people who prefer not to use the hosted badge. The Action takes `badge-style: static`.

## 1.2.0

- **Database map.** `repomap db` and the `db` MCP tool map tables, columns, keys and indexes, and link every table to the code that queries it (raw SQL, Prisma, Drizzle, SQLAlchemy, Diesel).
  - Sources: SQL migrations (in order, rollbacks skipped), Prisma, Drizzle, SQLAlchemy and Flask-SQLAlchemy, and Diesel.
  - Live Postgres, MySQL and SQLite are strictly read-only:
    - Postgres uses a read-only transaction and verifies it before reading anything.
    - MySQL uses `START TRANSACTION READ ONLY`.
    - SQLite opens the file read-only with `query_only`.
    - Only catalog metadata is read.
    - The connection string comes from an argument or an env var and is never stored or printed.
  - `--serve` / `--out` render tables and foreign keys in the graph UI, with a "Queried from" panel.
- **Agent-readiness score.** `repomap score` rates a repository from 0 to 100 on eight checks and lists concrete fixes.
  - The checks: agent instructions, build/test commands, tests, CI, module boundaries, file sizes, docs and types.
  - It prints a README badge (`mark.sylphx.com/badge/agent--ready-…`).
  - Flags: `--min` gates CI; `--update-readme` refreshes the badge.
- **GitHub Action.** `uses: SylphxAI/repomap@v1` scores on CI, writes the job summary, and can keep the README badge current.

## 1.1.2

- **Docker image** `ghcr.io/sylphxai/repomap` (linux/amd64, linux/arm64), published on every release. It runs the stdio MCP server; mount the repo at `/workspace`.
- The MCP server skips client roots that do not exist locally (container clients see host paths), falling back to the working directory.
- The release workflow marks the old MCP Registry names `io.github.SylphxAI/spine` and `io.github.SylphxAI/locus` as deprecated, pointing at repomap.

## 1.1.1

- `setup --claude-hooks` run through `npx` no longer writes `repomap hook` (a temporary npx shim) into Claude Code settings. It writes `npx -y @sylphx/repomap hook` unless repomap is installed globally.

## 1.1.0

- **Better modules.** Tests, examples, docs and benchmarks are detected by path (including `jvmTest`, `runtime-tests`, `watchOS Example`) and grouped by role. They no longer join or name core modules. Modules are named after their dominant directory with generic segments dropped (`tokio/runtime`, `flask/sansio`, `okhttp/okhttp3`). Name clashes are settled by the sub-directory that sets a module apart, or by its central file. Unconnected stragglers go into one `other` group instead of many one-file modules.
- **Kotlin and Swift** now get a symbol graph: definitions, calls, imports (Kotlin) and inheritance.
- **Claude Code hook.** `repomap setup --claude-hooks` installs an opt-in PreToolUse hook that adds repomap context (definition, callers, module) to Grep and Glob. `repomap hook` is the command; `setup --remove` removes it.
- **Graph UI polish:**
  - Selecting a file no longer over-zooms, and it stays clear of the side panel.
  - The canvas sits beside the module legend.
  - Search is centred over the free space.
  - Palette colours are more distinct.
  - Tests, examples and docs are muted and hidden by default in bigger repos.
- **Live demo:** `/demo` on the docs site hosts exported maps of excalidraw, axum and flask, rebuilt by CI.
- **README demo GIF**, recorded from the real UI (`scripts/record-demo.py`).

## 1.0.1

- Old launch commands keep working: `npx @sylphx/locus --root=/repo` (flags with no subcommand) starts the MCP server, and `LOCUS_ROOT`/`CODERAG_ROOT` are honoured.
- `map --limit` now also caps modules and entry points.
- `setup --dry-run` says what *would* change.
- Graph UI: impact view labels only the selected file and the central direct dependents.
- Release: publish every package first, then wait once for the npm registry, so a slow registry does not fail the run.

## 1.0.0

repomap is Spine and Locus, merged and rebuilt.

- New engine: tree-sitter extraction for TypeScript, TSX, JavaScript, Python, Go,
  Rust, Java, C, C++, C#, Ruby and PHP; resolved import, call and inheritance
  graph; PageRank; Louvain modules; BM25 over AST chunks; per-file cache; and
  `.gitignore` support.
- Five MCP tools: `map`, `search`, `context`, `trace` and `impact` (including
  `changed: true` for the current git diff). The old `architecture_*` and
  `codebase_search` names are still accepted until 2.0.
- `repomap serve`: an interactive WebGL graph UI with search, an impact and
  dependency view, and code preview. `repomap export`: a self-contained HTML map.
- `repomap setup`: one command that configures Claude Code, Codex, Cursor,
  VS Code, Claude Desktop, Windsurf and Gemini CLI.
- Native binaries for macOS (arm64, x64), Linux glibc (x64, arm64) and
  Windows x64.
- `@sylphx/spine`, `@sylphx/locus` and `@sylphx/coderag` are now aliases of
  `@sylphx/repomap`.
