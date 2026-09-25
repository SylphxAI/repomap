# CLI

```text
repomap setup [--client a,b] [--dry-run] [--remove]   configure MCP clients
repomap serve [dir] [--port 7878] [--host 127.0.0.1] [--no-open]
repomap export [dir] [--out repomap.html] [--json]    self-contained HTML (or graph JSON)
repomap map [focus-dir] [-C root] [--limit N] [--json]
repomap search <query…> [--path P] [--kind K] [--limit N] [--json]
repomap context <target> [--code-lines N] [--json]
repomap trace <from> [to] [--callers] [--depth N] [--json]
repomap impact [target…] [--changed] [--base REF] [--depth N] [--json]
repomap index [dir] [--no-cache] [--json]            build and print timings
repomap mcp [--root dir]                             MCP server on stdio
repomap version
```

`-C/--root` sets the repository for the query commands (the default is the current directory). With no arguments, and stdin not a terminal, `repomap` runs the MCP server.
