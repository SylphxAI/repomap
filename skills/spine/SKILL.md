# Spine — repository architecture with file-level proof

```bash
npx @sylphx/spine
```

`architecture_index` defaults to mode `refresh`: cache hit, incremental, or full as needed. `auto` is the old name for that same behavior. `full` is an explicit rescan. `status_only` checks freshness and does not index.

Primary: index, status, overview/search, path, impact. Advanced: trace, evidence, context_pack.
