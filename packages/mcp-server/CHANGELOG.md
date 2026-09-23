# @sylphx/architecture-reader-mcp

## 0.4.5

### Patch Changes

- 32e0472: Symbol evidence uses the line the match is on, including a match at column 0. Tool envelopes and the MCP server version follow `packages/mcp-server/package.json` instead of the private root package or a stale fallback.

## 0.4.4

### Patch Changes

- 7f1af75: `architecture_index` now defaults to `refresh`: a cache hit, an incremental update, or a full scan, whichever the index needs. `auto` remains the old name for that same behavior. `full` is still an explicit rescan, and `status_only` still does not index.

## 0.4.1

### Patch Changes

- b89e216: The published manifest gains real `keywords` (code analysis, repository analysis, impact analysis, call graph, AST, agent context, and the family terms), so npm search can surface the package for the queries its users type.
- b89e216: The MCP server runs the architecture engine in-process.

  Every published tool failed for anyone who installed the package: the server spawned a separate `architecture-reader-cli` binary that the npm package never shipped, so each call answered "architecture-reader-cli is unavailable". The engine already lives in `architecture-reader-core`, which the server links, so tools now call it directly. The package is self-contained and every call is faster.

## 0.2.0

### Minor Changes

- 4ce0a71: Initial public npm publish with Rust-default rmcp transport and commercial-grade README surfaces.
