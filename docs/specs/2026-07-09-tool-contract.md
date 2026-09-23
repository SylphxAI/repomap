# Tool Contract Spec

## Response Envelope

All tools return the same top-level envelope:

```json
{
  "status": "ok",
  "repository": {
    "root": "/abs/path",
    "indexedCommit": "abc123",
    "currentCommit": "abc123",
    "freshness": "fresh"
  },
  "answer": {},
  "evidence": [],
  "gaps": [],
  "metrics": {
    "elapsedMs": 12,
    "nodeCount": 100,
    "edgeCount": 250
  }
}
```

`freshness` values:

- `fresh`: index commit equals current clean checkout commit.
- `stale`: checkout commit differs from index commit.
- `dirty`: working tree has uncommitted changes.
- `unknown`: git state could not be determined.

## Evidence Reference

```json
{
  "id": "ev_01",
  "kind": "ast",
  "path": "src/server.ts",
  "startLine": 10,
  "endLine": 42,
  "extractor": "parser-substrate-typescript@0.3.x",
  "confidence": "deterministic"
}
```

`confidence` values:

- `deterministic`
- `derived`
- `inferred`
- `conflicting`
- `unknown`

## Tools

### `architecture_index`

Create or refresh the architecture index.

Input:

```json
{
  "root": "/abs/path",
  "mode": "refresh",
  "include": ["src", "docs"],
  "exclude": ["node_modules", "dist"],
  "maxFileBytes": 1048576
}
```

Omitting `mode` is the same as `refresh`. The response reports which branch ran: `cache_hit`, `incremental`, or `full`.

Modes:

- `refresh`: reuse the index, update what changed, or scan fully when required.
- `auto`: the old name for `refresh`. Same behavior.
- `full`: explicit complete rescan.
- `status_only`: check freshness and do not index.
- anything else: complete rescan.

Output answer:

```json
{
  "indexed": true,
  "filesScanned": 120,
  "filesIndexed": 95,
  "nodes": 430,
  "edges": 910,
  "coverage": {
    "ast": 0.76,
    "manifests": 1,
    "docs": 0.88
  }
}
```

### `architecture_status`

Report index state, freshness, coverage, extractor versions, and known gaps.

Input:

```json
{
  "root": "/abs/path"
}
```

### `architecture_overview`

Return a compact architecture map.

Input:

```json
{
  "root": "/abs/path",
  "scope": "repo",
  "depth": 2,
  "focus": "runtime"
}
```

`focus` values: `runtime`, `data`, `api`, `package`, `delivery`, `docs`,
`all`.

### `architecture_search`

Search architecture nodes, edges, and claims.

Input:

```json
{
  "root": "/abs/path",
  "query": "where is auth enforced?",
  "types": ["service", "module", "route", "schema", "decision"],
  "limit": 10,
  "includeEvidence": true
}
```

Search combines structural filters, lexical terms, graph neighborhood ranking,
and optional semantic retrieval.


### `architecture_path`

Shortest path between two architecture entities with hop-level provenance.

Request:

```json
{
  "root": "/abs/path",
  "from": "authMiddleware",
  "to": "validateToken",
  "relation": "calls",
  "maxDepth": 8
}
```

Aliases: `source`/`target` for `from`/`to`.

Answer includes:

- `nodes`: ordered node ids
- `hops`: `{ from, to, edgeId, edgeKind, provenance }` where `provenance` is `extracted` or `inferred`
- `hopCount`, resolved `fromId`/`toId`

This is the Graphify-style path query; `architecture_trace` remains available for relation-oriented traces.

### `architecture_trace`

Trace paths between architecture entities.

Input:

```json
{
  "root": "/abs/path",
  "from": "src/api/auth.ts",
  "to": "users table",
  "relation": "any",
  "maxDepth": 5
}
```

Relations: `imports`, `calls`, `routes`, `reads_from`, `writes_to`,
`depends_on`, `documents`, `tests`, `any`.

### `architecture_impact`

Estimate the architecture impact of changed files, symbols, or a git diff.

Input:

```json
{
  "root": "/abs/path",
  "changedPaths": ["src/api/auth.ts"],
  "includeTests": true,
  "includeDocs": true
}
```

Output should separate direct impact, transitive impact, unknown impact, and
recommended verification commands when the graph has evidence for them.

### `architecture_evidence`

Fetch exact evidence for a node, edge, claim, or search result.

Input:

```json
{
  "root": "/abs/path",
  "ids": ["node:service:api", "edge:imports:api:auth"],
  "maxBytes": 12000
}
```

## Error Shape

Errors use the same envelope with `status: "error"`:

```json
{
  "status": "error",
  "code": "INDEX_NOT_FOUND",
  "message": "No architecture index exists for this repository.",
  "nextAction": "Call architecture_index with mode refresh. auto is the old name for the same behavior."
}
```

Standard codes:

- `INVALID_ROOT`
- `INDEX_NOT_FOUND`
- `INDEX_STALE`
- `UNSUPPORTED_QUERY`
- `EXTRACTOR_FAILED`
- `EVIDENCE_NOT_FOUND`
- `INTERNAL_ERROR`
