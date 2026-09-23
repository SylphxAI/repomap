# Tool surface — Spine

Spine is repository architecture with file-level proof. The public workflow stays small.

## Primary

| Tool | Use it when |
| --- | --- |
| `architecture_index` | You need a graph. Omit `mode` to reuse the index, update what changed, or scan fully when required. |
| `architecture_status` | You need freshness and coverage, not a new scan. |
| `architecture_overview` | You need the map: boundaries, fan-in, fan-out, cycles. |
| `architecture_explain` | You want that map plus the next useful question. |
| `architecture_search` | You know a symbol, route, or path fragment and need the file. |
| `architecture_path` | You need the shortest path between two nodes. |
| `architecture_impact` | You are about to edit a file and need the blast radius. |

## Advanced

| Tool | Use it when |
| --- | --- |
| `architecture_trace` | Path is not enough and you need a call or dependency walk. |
| `architecture_evidence` | You already have an evidence id and need the file:line record. |
| `architecture_context_pack` | You need a packed neighborhood. Do not start here. |

Lead with index, then status, then overview or search. Locus owns chunk retrieval. Spine owns architecture claims.
