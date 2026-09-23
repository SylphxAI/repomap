---
"@sylphx/spine": patch
---

`architecture_index` now defaults to `refresh`: a cache hit, an incremental update, or a full scan, whichever the index needs. `auto` remains the old name for that same behavior. `full` is still an explicit rescan, and `status_only` still does not index.
