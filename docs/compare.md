# Comparison

| | **repomap** | GitNexus | Serena | claude-context | Aider repo map |
|---|---|---|---|---|---|
| Licence | **MIT** | PolyForm Noncommercial | MIT | MIT | Apache-2.0 (inside Aider) |
| Install | `npx … setup`, one native binary | `npx`, Node | Python + language servers | Vector DB + embedding API key | Part of Aider |
| Network / API key | **None** | None for the graph | None | Required | None |
| Call + import + inheritance graph | ✅ tree-sitter | ✅ tree-sitter | via LSP | — | tags + ranking |
| Change impact incl. `git diff` | ✅ | ✅ | — | — | — |
| Call path between symbols | ✅ | ✅ | — | — | — |
| Search | BM25 over AST chunks + symbol names | ✅ | symbol lookup | semantic (embeddings) | — |
| Graph UI | ✅ local + static HTML export | ✅ in browser | — | — | — |
| Communities / modules | ✅ Louvain | ✅ | — | — | — |
| Edits code | — | — | ✅ | — | ✅ |
| Engine | Rust | TypeScript | Python | TypeScript | Python |

**When to choose something else.** Use Serena for LSP-exact rename and refactor edits. Use claude-context for semantic, meaning-level search when you can run an embedding model. GitNexus adds an LLM chat over its graph, but its licence rules out commercial use.

**Why repomap.** It needs no setup and runs on any machine. It answers the questions agents actually ask (where, what, how it connects, what breaks) with `file:line` citations and few tokens, and it produces a map you can show your team. The licence lets you use it at work.
