# Vision — Spine

Spine is the repository architecture tool for agents.

- **Identity:** package `@sylphx/spine`, bin `spine`, MCP `io.github.SylphxAI/spine`, site <https://sylphxai.github.io/spine/>.
- **User:** an agent or developer who must understand a repository before editing it.
- **Job:** answer what the repository is, where a behavior lives, how a request flows, and what a change affects, with file:line evidence.
- **Promise:** deterministic local graph and answer envelopes; freshness, coverage, confidence and gaps are explicit; no cloud code-intel account and no generative prose as truth.
- **Defaults:** `mode: full` builds a complete graph; `mode: status_only` checks freshness; `mode: auto` is a compatibility alias for incremental refresh.
- **Boundaries:** Spine owns architecture claims — boundaries, paths, traces, impact. Locus owns code-chunk retrieval; neither edits files.
