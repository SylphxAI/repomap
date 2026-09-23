# Tool reference

Start with the smallest tool that answers the question. Index first. Then ask.

| Tool | What you pass | What you get |
| --- | --- | --- |
| `architecture_index` | `root`, optional `mode` | A local graph. Omitted mode is `refresh` (cache hit, incremental, or full). `auto` is the old name. `full` rescans. `status_only` does not index. |
| `architecture_status` | `root` | Freshness, extractors, coverage, and gaps for the current index |
| `architecture_overview` | `root`, optional `focus` and `depth` | Packages, modules, boundaries, fan-in, fan-out, and cycles |
| `architecture_explain` | `root` | The overview plus the most useful next questions |
| `architecture_search` | `root`, `query` | Ranked nodes with path and line evidence |
| `architecture_path` | `root`, `from`, `to` | Shortest path and hop provenance (`extracted` or `inferred`) |
| `architecture_trace` | `root`, `from`, `to` | A dependency or call path |
| `architecture_impact` | `root`, `changedPaths` or a git diff | Incoming dependents, outgoing dependencies, and unknown impact |
| `architecture_evidence` | `root`, `ids` | The file and line records behind a claim |
| `architecture_context_pack` | `root`, `focus` | Advanced: a neighborhood pack. Not the first call. |

Each result keeps the repository root, indexed and current commit, freshness, evidence locators, warnings, and explicit gaps.
