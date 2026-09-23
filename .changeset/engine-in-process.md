---
"@sylphx/spine": patch
---

The MCP server runs the architecture engine in-process.

Every published tool failed for anyone who installed the package: the server spawned a separate `architecture-reader-cli` binary that the npm package never shipped, so each call answered "architecture-reader-cli is unavailable". The engine already lives in `architecture-reader-core`, which the server links, so tools now call it directly. The package is self-contained and every call is faster.
