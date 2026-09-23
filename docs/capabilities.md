# Capabilities — Spine

## Surfaces

| Surface | Identity |
| --- | --- |
| MCP | `io.github.SylphxAI/spine` over stdio, `npx -y @sylphx/spine` |
| CLI | `spine` |
| SDK | `@sylphx/spine/sdk` |

## Owned capabilities

| Capability | Tool | Evidence |
| --- | --- | --- |
| Graph build/refresh | `architecture_index` | omitted mode `refresh`: cache hit, incremental, or full; repository root, indexed commit, coverage |
| Freshness/status | `architecture_status` | current commit, freshness, relation histogram, gaps |
| Repository map | `architecture_overview` | ranked nodes, boundaries, cycles, fan-in/fan-out |
| Architecture explanation | `architecture_explain` | map summary and useful next questions |
| Boundary search | `architecture_search` | ranked nodes with path and line evidence |
| Path between nodes | `architecture_path` | hop provenance (`extracted`/`inferred`) |
| Impact analysis | `architecture_impact` | incoming dependents, outgoing dependencies, unknown impact |
| Dependency trace | `architecture_trace` | dependency or call-path evidence |
| Evidence lookup | `architecture_evidence` | file:line proof behind a claim |

## Evidence contract

Every answer carries repository root, indexed/current commit, freshness, extraction source, confidence and gaps. See [EVIDENCE_CONTRACT.md](./EVIDENCE_CONTRACT.md).

## Not owned

Code-chunk retrieval, filesystem mutation, cloud code-intel, and generative summaries as architecture truth. Tree-sitter is not the default graph. Import and call extraction is a regex fallback (`importGraphRoute=regex_fallback`) unless `ARCHITECTURE_READER_USE_SYNTH=1` turns on Synth AST.
