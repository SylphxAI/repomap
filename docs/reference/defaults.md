# Predictable defaults

An omitted `architecture_index` uses `refresh`: a cache hit, an incremental update, or a full scan when one is required. A full rescan is mode `full`, named explicitly. There is no model call on this path.

| Mode | Behavior |
| --- | --- |
| omitted, or `refresh` | Cache hit, incremental update, or full scan, as needed. The response reports `cache_hit`, `incremental`, or `full` — not the input name. |
| `auto` | The old name for `refresh`. Same behavior. |
| `full` | Force a complete rescan. |
| `status_only` | Check freshness and coverage without indexing. |
| anything else | Force a complete rescan. |

Unknown languages and incomplete graph coverage are returned as gaps.
