# repomap

A map of your codebase for AI agents and people: a code graph, hybrid search,
call paths and change impact, with an interactive graph UI. It ships as a Rust
MCP server and CLI, runs locally, needs no API key, and is MIT licensed.

- Lifecycle: `active`, published as `@sylphx/repomap` (npm) and
  `io.github.SylphxAI/repomap` (MCP Registry)
- Docs: https://sylphxai.github.io/repomap/
- Formerly Spine (`@sylphx/spine`) and Locus (`@sylphx/locus`,
  `@sylphx/coderag`). Those packages are now thin aliases.

## Layout

- `crates/repomap-core`: walk, tree-sitter extraction, import and call
  resolution, PageRank, Louvain, BM25, and the queries (map, search, context,
  trace, impact)
- `crates/repomap`: the `repomap` binary (CLI, MCP stdio server, `serve` HTTP
  UI, `export`, `setup`)
- `ui/`: graph UI source (TypeScript, Sigma.js). `bun run build:ui` writes
  `crates/repomap/assets/`, which is committed so cargo builds need no JS
  toolchain
- `packages/repomap`: the npm launcher; `packages/npm/*`: the native binaries;
  `packages/aliases/*`: the spine, locus and coderag aliases
- `docs/`: the VitePress site; `scripts/`: version sync and the benchmark

## Release

Bump the version with `bun scripts/set-version.ts X.Y.Z && cargo update -w`,
then merge. `release.yml` publishes when the npm version is new: it builds 5
native targets on GitHub-hosted runners, publishes the natives, the main
package and the aliases, smoke-tests with `npx`, creates the GitHub release,
and publishes to the MCP Registry.
