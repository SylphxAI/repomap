# Predictable defaults

| Mode | Behavior |
| --- | --- |
| `full` | Build the complete local architecture graph |
| `status_only` | Check repository freshness and coverage without indexing |
| `auto` | Compatibility alias for incremental refresh |

No remote provider or model is required. Unsupported languages and incomplete
graph coverage are returned as gaps.
