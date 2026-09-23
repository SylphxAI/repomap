# Quickstart

## Install

```bash
npx -y @sylphx/spine
```

For Claude Code:

```bash
claude mcp add spine -- npx -y @sylphx/spine
```

Any MCP client uses the same stdio server:

```json
{
  "mcpServers": {
    "spine": { "command": "npx", "args": ["-y", "@sylphx/spine"] }
  }
}
```

## Refresh the graph

Omit `mode`. Spine reuses the index, updates what changed, or scans fully when it has to. The response names that as `cache_hit`, `incremental`, or `full`.

```json
{
  "root": "/absolute/path/to/repo"
}
```

`refresh` is the name of that default. `auto` is the old name for the same behavior. A complete rescan is explicit:

```json
{
  "root": "/absolute/path/to/repo",
  "mode": "full"
}
```

`status_only` checks freshness and does not index. There is no model call and no API key on this path.

## Ask one question

```json
{
  "query": "authMiddleware"
}
```

Search matches a label or a path fragment, not a sentence. Read the locators, the extraction label, the freshness, and the gaps before treating the answer as true.
