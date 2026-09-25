<div align="center">

# repomap

**A map of your codebase for you and your AI agent.**

Code graph · hybrid search · call paths · change impact · an interactive graph UI.<br>
One Rust binary. Local. No API key. MIT.

[![npm](https://img.shields.io/npm/v/@sylphx/repomap?color=8aa4ff&label=npm)](https://www.npmjs.com/package/@sylphx/repomap)
[![CI](https://github.com/SylphxAI/repomap/actions/workflows/ci.yml/badge.svg)](https://github.com/SylphxAI/repomap/actions/workflows/ci.yml)
[![MCP Registry](https://img.shields.io/badge/MCP%20Registry-io.github.SylphxAI%2Frepomap-42d6a4)](https://registry.modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/license-MIT-ffb454)](LICENSE)

[Docs](https://sylphxai.github.io/repomap/) · [Quickstart](#quickstart) · [Tools](#what-your-agent-gets) · [Graph UI](#the-graph-ui) · [Benchmarks](https://sylphxai.github.io/repomap/benchmarks) · [Compare](#how-it-compares)

<img src="docs/public/img/hero-excalidraw.webp" alt="repomap graph UI showing the excalidraw repository: files clustered into modules, sized by centrality" width="100%">

<sub>excalidraw, 687 files, indexed in under half a second. Every dot is a file, colours are modules found from the dependency graph, and size is PageRank centrality.</sub>

</div>

## Quickstart

```bash
npx -y @sylphx/repomap setup     # add repomap to Claude Code, Codex, Cursor, VS Code, Claude Desktop, Windsurf, Gemini CLI
npx -y @sylphx/repomap serve     # open the graph UI for the current repo
```

That's it. `setup` detects the clients you have, writes their MCP config, and prints every change it made. Run it again and nothing changes. Then ask your agent: *"Use repomap to map this repo."*

<details>
<summary>Manual MCP config</summary>

```json
{
  "mcpServers": {
    "repomap": { "command": "npx", "args": ["-y", "@sylphx/repomap", "mcp"] }
  }
}
```

Claude Code: `claude mcp add repomap -- npx -y @sylphx/repomap mcp`
Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.repomap]
command = "npx"
args = ["-y", "@sylphx/repomap", "mcp"]
```

The server indexes the client's workspace root (or its working directory, or `REPOMAP_ROOT`). Every tool also takes `root`.
</details>

## Why

Agents burn most of their context on `grep`, `ls` and reading whole files just to work out where things are. repomap gives them the map up front:

- **Where is it?** Hybrid search that matches symbol names *and* the words inside functions, with the lines that matched.
- **What is this?** One call returns a symbol's code, callers with call-site lines, callees, subtypes and the tests that reach it.
- **How does A reach B?** The shortest call path, each hop cited `file:line`.
- **What breaks if I change this?** Direct and indirect callers, importing files, modules touched, tests to run, and a risk level. Point it at your `git diff` before you commit.
- **What does this repo look like?** Modules found from real dependencies (not just folders), the most central files, the most used symbols, and entry points.

All of it comes from a local index: tree-sitter parsing, a resolved import and call graph, PageRank, Louvain communities and BM25 over AST chunks. Nothing leaves your machine, and no model or embedding API is called.

## What your agent gets

Five tools, each with an obvious job:

| Tool | Ask it | Returns |
|---|---|---|
| `map` | "Give me the lay of the land" / `focus: "src/server"` | Modules, central files, key symbols, entry points; an outline with line numbers when focused |
| `search` | `"refresh token expiry"`, `"parseConfig"` | Ranked `file:line` ranges (functions, methods, classes) with the matching lines |
| `context` | `SessionStore.refresh`, `src/auth/token.ts`, `token.ts:42` | Code, callers (with call sites), callees, subtypes, members, imports, importers, tests |
| `trace` | `from: handleRequest, to: db.query` | Shortest call path, or the call tree above/below a symbol |
| `impact` | `target: verifyToken` or `changed: true` | Risk level, callers by depth, importing files, modules, tests to run |

Answers are compact text that cites `file:line`, so they cost few tokens. Pass `format: "json"` for structured output.

```text
> impact target=decode

# Impact (LOW risk)
Changing: function decode (src/auth/token.ts:14)
1 direct caller, 3 symbols affected in total across 3 files and 2 modules; 1 file importing the changed files; no tests reach this.

## Direct callers (will break if the contract changes)
- function verifyToken — src/auth/token.ts:8

## Indirect (depth 2)
- method SessionStore.refresh — src/auth/session.ts:5

## Indirect (depth 3)
- function handleRefresh — src/api/router.ts:6

## Importing files
- src/auth/session.ts
```

The same five commands work in your terminal: `repomap map`, `repomap search "…"`, `repomap context X`, `repomap trace A B`, `repomap impact --changed`.

## The graph UI

```bash
npx -y @sylphx/repomap serve            # live, with code preview
npx -y @sylphx/repomap export           # repomap.html: one self-contained file you can publish
```

<table>
<tr>
<td width="50%"><img src="docs/public/img/impact-tokio.webp" alt="Impact view on tokio: dependents of runtime/task/mod.rs highlighted by depth"></td>
<td width="50%"><img src="docs/public/img/code-panel-tokio.webp" alt="Code panel on tokio: harness.rs poll() with its callers and callees"></td>
</tr>
<tr>
<td><b>Impact.</b> Select a file and press <kbd>i</kbd>: everything that depends on it lights up by depth, with a list you can click through.</td>
<td><b>Code.</b> Click a symbol to see its source, callers and callees. <b>Open ↗</b> jumps to your editor or to GitHub.</td>
</tr>
</table>

- WebGL rendering (Sigma.js) that stays smooth with tens of thousands of files and edges
- Modules coloured and clustered by their real dependencies, file size by PageRank
- <kbd>/</kbd> searches files, symbols and code; <kbd>d</kbd> shows dependencies; <kbd>f</kbd> fits the view
- Click a module to focus it, <kbd>Shift</kbd>-click to hide it; toggle tests, edges and labels
- `export` writes one HTML file with deep links (`#path/to/file`) and GitHub links pinned to your commit. It's a good fit for a README, a wiki or a design review.

## Languages

Parsed with tree-sitter for symbols, calls, imports and inheritance: **TypeScript, TSX, JavaScript, Python, Go, Rust, Java, C, C++, C#, Ruby, PHP.**
Search also covers Markdown, YAML, TOML, JSON, SQL, shell, Protobuf, GraphQL, HTML/CSS, Vue, Svelte, Kotlin, Swift, Scala and more.

Import resolution understands relative paths, `@/` aliases, npm workspace packages, Python packages and relative imports, Go modules, Rust `mod`/`use`/workspace crates, Java/PHP namespaces, C/C++ includes and Ruby `require`. `.gitignore` is respected, and so is `.repomapignore`.

## Fast

The index is built in parallel and cached per file, so after the first run only changed files are parsed again. The MCP server keeps the graph in memory and refreshes it when files change.

Measured on a 4 vCPU GitHub-hosted runner ([method and full table](https://sylphxai.github.io/repomap/benchmarks)):

| Repository | Code files | Cold index | Warm index | search p50 | impact p50 |
|---|---:|---:|---:|---:|---:|
| kubernetes | 11,710 | 11.2 s | 1.9 s | 57 ms | 5 ms |
| vscode | 6,125 | 8.0 s | 1.3 s | 7 ms | 6 ms |
| django | 2,271 | 2.2 s | 0.4 s | 21 ms | 1 ms |
| rust-analyzer | 1,512 | 1.8 s | 0.3 s | 4 ms | 2 ms |

## How it compares

| | **repomap** | GitNexus | Serena | claude-context | Aider repo map |
|---|---|---|---|---|---|
| Licence | **MIT** | PolyForm Noncommercial | MIT | MIT | Apache-2.0 (inside Aider) |
| Setup | `npx … setup`, one binary | `npx`, Node | Python + language servers | Vector DB + embedding API key | Part of Aider |
| API key / network | **None** | None for the graph | None | Required (embeddings) | None |
| Code graph (calls, imports, inheritance) | ✅ | ✅ | Via LSP references | — | Ranking only |
| Change impact / blast radius | ✅ incl. `git diff` | ✅ | — | — | — |
| Call path between two symbols | ✅ | ✅ | — | — | — |
| Keyword + symbol search | ✅ BM25 + names | ✅ | ✅ symbols | Semantic (vectors) | — |
| Interactive graph UI | ✅ local + static export | ✅ | — | — | — |
| Module detection | ✅ Louvain | ✅ | — | — | — |
| Edits code | — (read-only) | — | ✅ | — | ✅ |
| Engine | Rust | TypeScript | Python | TypeScript | Python |

Pick Serena if you want LSP-precise refactoring edits, and claude-context if you want embedding-based semantic search. repomap is for understanding and navigating a codebase with zero setup, and a licence you can use at work.

## CLI

```text
repomap setup [--client cursor,codex] [--dry-run] [--remove]
repomap serve [dir] [--port 7878] [--no-open]
repomap export [dir] [--out repomap.html] [--json]
repomap map [dir-to-focus] [-C root] [--json]
repomap search <query> [--path src/] [--kind function] [--limit 10]
repomap context <target> [--code-lines 60]
repomap trace <from> [to] [--callers] [--depth 3]
repomap impact [targets…] [--changed] [--base main]
repomap index [dir] [--no-cache] [--json]
repomap mcp [--root dir]
```

Binaries for macOS (arm64, x64), Linux glibc (x64, arm64) and Windows x64 ship as npm optional dependencies. They're also attached to each [GitHub release](https://github.com/SylphxAI/repomap/releases). From source: `cargo install --git https://github.com/SylphxAI/repomap repomap`.

## Formerly Spine and Locus

repomap merges **Spine** (architecture graph) and **Locus** (BM25 code search, formerly CodeRAG) into one tool. `@sylphx/spine`, `@sylphx/locus` and `@sylphx/coderag` still install and run repomap. The old tool names (`architecture_*`, `codebase_search`) are still accepted until 2.0, but new setups should use `@sylphx/repomap`.

## Contributing

```bash
cargo test --workspace          # engine + CLI tests
bun install && bun run build:ui # rebuild the UI bundle (ui/ -> crates/repomap/assets/)
cargo run -p repomap -- serve .
```

Issues and PRs are welcome, and new language support is especially useful: add a grammar and a query in `crates/repomap-core/src/lang.rs`.

## Star history

[![Star History Chart](https://api.star-history.com/svg?repos=SylphxAI/repomap&type=Date)](https://star-history.com/#SylphxAI/repomap&Date)

MIT © Sylphx
