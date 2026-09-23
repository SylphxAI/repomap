# Spine

### Repository architecture with file-level proof

Spine gives an agent a local map of a repository before it edits:

- **What is this repository?**
- **Where does this behavior live?**
- **How does a request reach that module?**
- **What does this change affect?**
- **Which file and lines support the answer?**

The graph stays on the machine. Spine does not call a model, and it does not treat generated prose as proof.

```bash
npx -y @sylphx/spine
```

This starts the MCP server on stdio. For Claude Code:

```bash
claude mcp add spine -- npx -y @sylphx/spine
```

Package [`@sylphx/spine`](https://www.npmjs.com/package/@sylphx/spine) · bin `spine` · MCP `io.github.SylphxAI/spine` · docs <https://sylphxai.github.io/spine/>

## The default is a refresh

Omit `mode`. Spine reuses the index when nothing changed, updates only the files that changed when the delta is small, and scans the repository when it has to. The response names which one happened: `cache_hit`, `incremental`, or `full`.

```json
{
  "root": "/absolute/path/to/repo"
}
```

A complete rescan is named explicitly:

```json
{
  "root": "/absolute/path/to/repo",
  "mode": "full"
}
```

- `refresh` is the default. `auto` is the old name for that same behavior.
- `status_only` checks freshness and does not index.
- Any other mode forces a full scan.
- No remote provider and no model call.
- Unknown languages and incomplete coverage come back as gaps.

## Ask one question

```json
{
  "query": "authMiddleware"
}
```

Search matches a label or a path fragment, such as a symbol name. It returns ranked nodes with `path`, line range, extraction source, freshness, confidence, and known gaps.

## Jobs Spine is built for

| Ask your agent | Spine returns |
| --- | --- |
| “Map this repository.” | `architecture_overview` |
| “Where is this implemented?” | `architecture_search` |
| “How does this request reach the database?” | `architecture_trace` |
| “What breaks if I edit this file?” | `architecture_impact` |
| “Show me the proof.” | `architecture_evidence` |

## Tool surface

| Tool | Purpose |
| --- | --- |
| `architecture_index` | Build or refresh the local graph. Default mode is `refresh`. |
| `architecture_status` | Report freshness, coverage, and gaps |
| `architecture_overview` | Repository map and important boundaries |
| `architecture_explain` | Map plus the most useful next questions |
| `architecture_search` | Find boundaries, routes, schemas, and symbols |
| `architecture_path` | Shortest path between two nodes, with hop provenance |
| `architecture_impact` | Estimate the blast radius of a change |
| `architecture_trace` | Follow a dependency or call path |
| `architecture_evidence` | Fetch the file and line proof for a claim |

`architecture_context_pack` is an advanced composition tool. Start with the question-oriented tools above.

## Why an agent can check the answer

Every result carries:

- repository root and indexed commit;
- current commit and freshness (`fresh`, `stale`, `dirty`, or `unknown`);
- evidence with file and line locators;
- extracted versus inferred labels;
- explicit gaps instead of a silent guess.

## What Spine is not

| Tool | What it is | What Spine does differently |
| --- | --- | --- |
| [Serena](https://github.com/oraios/serena) | A coding agent toolkit: semantic retrieval and editing, often through language servers | Spine does not edit files. It returns a local architecture graph: path, trace, impact, and file:line evidence. |
| [ripgrep](https://github.com/BurntSushi/ripgrep) | Fast literal search across file contents | Spine is not a line search. It indexes boundaries and relations, then answers path, trace, and impact. |
| [Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) | An interactive knowledge graph built by a multi-agent pipeline that uses a model | Spine's default path never calls a model. It returns path, trace, impact, and file:line evidence, not a guided tour. |
| Language servers | Definitions, references, and types for one language inside an editor | Spine is a repository graph for an agent: cross-file path, trace, impact, freshness, and gaps. |

Locus owns code-chunk retrieval. Spine owns architecture claims. Neither replaces the other.

## Companion MCP tools

| Product | Job |
| --- | --- |
| [Citra](https://github.com/SylphxAI/citra) | PDF answers with page-level proof |
| [Iris](https://github.com/SylphxAI/iris) | Image facts and pixel evidence |
| [Cue](https://github.com/SylphxAI/cue) | Video timelines and timestamp evidence |
| [Locus](https://github.com/SylphxAI/locus) | Exact code-chunk retrieval |
| [Lookout](https://github.com/SylphxAI/lookout) | Web research with source excerpts |

Each product is independent. Install only the tools the agent needs.

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
