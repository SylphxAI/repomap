# Changelog

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
