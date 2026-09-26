# How it works

1. **Walk.** The [`ignore`](https://docs.rs/ignore) crate walks the repo in parallel, the same engine ripgrep uses. It respects `.gitignore`, global excludes and `.repomapignore`, and also skips vendored and build directories, lockfiles, minified and generated files, and anything over 1 MB.
2. **Parse.** tree-sitter grammars run with one query per language. The queries capture definitions (with full extents and owners), bare and member calls with their receivers, imports, and `extends`/`implements`. Results are cached per file (mtime + size), so later runs only parse what changed.
3. **Resolve imports.** Language-aware resolvers cover relative paths, `@/` aliases, npm workspaces, Python packages, Go modules, Rust `mod`/`use`/crates, Java and PHP namespaces, C/C++ includes and Ruby `require`.
4. **Resolve calls.** A call binds to a definition in the same file first, then to one in the files it imports, then by package or directory, and finally to a unique global name. Member calls such as `obj.get()` never bind on a generic name alone, and `this.`/`self.` calls stay inside the owning type. Ambiguous calls are left unresolved rather than guessed.
5. **Rank and cluster.** Weighted PageRank over the file graph finds central files, and symbols are ranked by who calls them. Louvain modularity finds modules, which are named after the directory that holds most of their central files.
6. **Chunk and index.** Every function, method and class becomes a chunk, and large classes are split into their members. The rest of each file is split into fixed windows. Each chunk is indexed twice:
   - BM25: tokens split camelCase and snake_case, symbol names are boosted, and path terms count too.
   - An embedding: the chunk's path and code through [potion-code-16M-v2](https://huggingface.co/minishlab/potion-code-16M-v2), a static model distilled from CodeRankEmbed. A static model gives each token a fixed vector, so an embedding is a mean of vectors and a CPU embeds a large repository in about a second. The vectors are stored as int8 in the per-file cache. The model downloads once (33 MB); without it, search is keyword-only.
7. **Search.** Three lists are fused by reciprocal rank: symbol-name matches, BM25 and embedding similarity. Identifier-like queries weigh the first two more. Then:
   - Files whose name or folder matches the question are lifted.
   - A file with several matching chunks lifts its best one.
   - Tests, examples, docs and compatibility shims rank lower.
   - Each further chunk from the same file counts less, so results spread over files.
8. **Serve.** The MCP server holds the index in memory. It checks a cheap file fingerprint before each query and rebuilds only when something changed.

## Accuracy notes

repomap resolves calls statically. It does not see dynamic dispatch, reflection, dependency injection or string-based routing, and when a call could mean several definitions it leaves the call out rather than guess. `impact` therefore also lists **importing files**, which catches usage that the call graph misses. Treat call edges as strong hints with a `file:line` you can check, not as proof.
