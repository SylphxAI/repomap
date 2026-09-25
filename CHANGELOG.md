# Changelog

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
