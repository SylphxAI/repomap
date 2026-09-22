# Evidence contract — Spine

Result contract v1. Locators: `file:line`, path hops, node ids.
Gaps: unknown impact, incomplete index, unsupported language.
No `evidence_first` tool. No LLM required for graph authority.

## Implemented family wire fields (v1)

Every tool result includes:

- `envelope_version: "1"`
- `status`, `tool`, `product`, `product_version`
- `route` as `{ engine, path? }`
- `warnings` and `gaps` arrays (may be empty)
- domain payload (often also as top-level twin/results/answer for compatibility)

Schema: `SylphxAI/skills` `schemas/product-evidence-envelope.schema.json`.
