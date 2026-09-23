# Predictable defaults

An omitted `architecture_index` uses `refresh`: a cache hit, an incremental update, or a full scan when one is required. The response says which: `cache_hit`, `incremental`, or `full`.

That default mode is named `refresh`. `auto` is the old name for the same behavior. A full rescan is mode `full`, named explicitly. `status_only` checks freshness and does not index. Any other mode forces a full scan.

No remote provider. No model call. Unknown languages and incomplete coverage are returned as gaps.
