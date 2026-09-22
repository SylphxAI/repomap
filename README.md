# Spine

### Repository architecture with file-level proof

Spine answers the questions agents need before they edit a codebase:

- **What is this repository?**
- **Where does this behavior live?**
- **What does this change affect?**
- **Which files and symbols support the answer?**

Local-first, deterministic, and fast. No cloud code-intel account, no Docker
stack, and no generated prose presented as truth.

```bash
npx -y @sylphx/spine
```

This starts the MCP server on stdio. For Claude Code:

```bash
claude mcp add spine -- npx -y @sylphx/spine
```

## The fastest useful workflow

```json
{
  "root": "/absolute/path/to/repo",
  "mode": "full"
}
```

Then ask one focused question:

```json
{
  "query": "where is request authentication enforced?"
}
```

Spine returns ranked boundaries and implementation nodes with `path`,
`line range`, extraction source, freshness, confidence, and known gaps.

## Jobs Spine is built for

| Ask your agent | Spine returns |
| --- | --- |
| “Map this repository.” | `architecture_overview` |
| “Where is this implemented?” | `architecture_search` |
| “How does this request reach the database?” | `architecture_trace` |
| “What breaks if I edit this file?” | `architecture_impact` |
| “Show me the proof.” | `architecture_evidence` |

## Tool surface

The public workflow is intentionally small:

| Tool | Purpose |
| --- | --- |
| `architecture_index` | Build or refresh the local graph |
| `architecture_status` | Report freshness and coverage |
| `architecture_overview` | Repository map and important boundaries |
| `architecture_explain` | Explain the repository map and useful next questions |
| `architecture_search` | Find boundaries, routes, schemas, and symbols |
| `architecture_path` | Explain a path between two nodes |
| `architecture_impact` | Estimate change blast radius |
| `architecture_trace` | Follow dependency or call paths |
| `architecture_evidence` | Fetch file and line proof for a claim |

`architecture_context_pack` is an advanced composition tool. Start with the
question-oriented tools above.

## Predictable defaults

Spine does not hide expensive or ambiguous work behind “auto”.

- `mode: "full"` builds a complete local graph.
- `mode: "status_only"` checks freshness without indexing.
- `mode: "auto"` is retained as a compatibility alias for incremental refresh.
- No remote provider or model is required.
- Unknown languages and incomplete graph coverage are returned as gaps.

## Why agents trust the answer

Every result carries:

- repository root and indexed commit;
- current commit and freshness (`fresh`, `stale`, `dirty`, or `unknown`);
- evidence with file and line locators;
- deterministic versus inferred extraction labels;
- explicit gaps instead of silent guesses.

## Product boundaries

Spine owns **architecture claims**:

- boundaries;
- paths and traces;
- impact;
- repository structure.

Locus owns code-chunk retrieval. Spine does not edit files, replace code
search, or require an LLM to explain its graph.

## Companion MCP tools

| Product | Job |
| --- | --- |
| [Citra](https://github.com/SylphxAI/citra) | PDF answers with page-level proof |
| [Iris](https://github.com/SylphxAI/iris) | Image facts and pixel evidence |
| [Cue](https://github.com/SylphxAI/cue) | Video timelines and timestamp evidence |
| [Locus](https://github.com/SylphxAI/locus) | Exact code-chunk retrieval |
| [Lookout](https://github.com/SylphxAI/lookout) | Web research with source excerpts |

Each product is independent. Install only the tools your agent needs.

## Development

```bash
bun install
bun run build
bun test
cargo test
```

For a release candidate:

```bash
bun run benchmark:public-proof
bun run benchmark:release-gate
```

## License

MIT
