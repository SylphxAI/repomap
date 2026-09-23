# Spine (`@sylphx/spine`)

### Repository architecture with file-level proof

Local graph, path, trace, and impact for agents. File and line evidence stays on the machine. No API key and no model call.

**npm** `@sylphx/spine` · **bin** `spine` · **MCP** `io.github.SylphxAI/spine` · **docs** <https://sylphxai.github.io/spine/>

Do not install the old package name `@sylphx/architecture-reader-mcp`.

```bash
npx -y @sylphx/spine
```

For Claude Code:

```bash
claude mcp add spine -- npx -y @sylphx/spine
```

## Default

Omit `mode` on `architecture_index`. That is `refresh`: a cache hit, an incremental update, or a full scan, and the response says which. `auto` is the old name for the same behavior. `full` forces a complete rescan. `status_only` checks freshness and does not index.

```json
{ "root": "/absolute/path/to/repo" }
```

## Tools

| Tool | Purpose |
| --- | --- |
| `architecture_index` | Build or refresh the local graph |
| `architecture_status` | Freshness, coverage, and gaps |
| `architecture_overview` | Repository map |
| `architecture_explain` | Map plus the next useful questions |
| `architecture_search` | Boundaries, routes, schemas, and symbols |
| `architecture_path` | Shortest path, with hop provenance |
| `architecture_impact` | Blast radius of a change |
| `architecture_trace` | Dependency or call path |
| `architecture_evidence` | File and line proof for a claim |

`architecture_context_pack` is advanced. Start with the tools above.

SDK entry: `@sylphx/spine/sdk`.
