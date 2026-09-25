# From Spine or Locus

repomap merges **Spine** (`@sylphx/spine`, architecture graph) and **Locus** (`@sylphx/locus`, formerly `@sylphx/coderag`, BM25 code search).

- `@sylphx/spine`, `@sylphx/locus` and `@sylphx/coderag` are now thin aliases. They install `@sylphx/repomap` and run it, so existing MCP configs keep working.
- The old tool names are still accepted until repomap 2.0:

| Old | New |
|---|---|
| `architecture_index`, `architecture_status`, `architecture_overview`, `architecture_explain` | `map` |
| `architecture_search`, `codebase_search` | `search` |
| `architecture_path`, `architecture_trace` | `trace` |
| `architecture_impact` | `impact` |
| `architecture_context_pack`, `architecture_evidence`, `find_related` | `context` |

- Answers are now compact text that cites `file:line`. The evidence envelopes are gone. Pass `format: "json"` for structured output.
- Locus's chunks were regex-based. repomap chunks with tree-sitter ASTs and adds symbol-name ranking, so the same queries return whole functions.

To switch, run `npx -y @sylphx/repomap setup` and remove the old `spine` or `locus` entries from your client config.
