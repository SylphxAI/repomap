# Quickstart

## Install

```bash
npx -y @sylphx/spine
```

For Claude Code:

```bash
claude mcp add spine -- npx -y @sylphx/spine
```

Then ask one concrete question and inspect the returned locators, route, warnings,
and gaps before relying on the answer.

## Predictable defaults

`mode: "full"` builds the complete local graph. Use `mode: "status_only"` to
check freshness without indexing. `mode: "auto"` remains an incremental-refresh
compatibility alias; it does not hide network or model work.
