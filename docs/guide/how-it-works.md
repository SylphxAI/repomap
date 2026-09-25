# How it works

1. **Walk.** The [`ignore`](https://docs.rs/ignore) crate walks the repo in parallel, the same engine ripgrep uses. It respects `.gitignore`, global excludes and `.repomapignore`, and also skips vendored and build directories, lockfiles, minified and generated files, and anything over 1 MB.
2. **Parse.** tree-sitter grammars run with one query per language. The queries capture definitions (with full extents and owners), bare and member calls with their receivers, imports, and `extends`/`implements`. Results are cached per file (mtime + size), so later runs only parse what changed.
3. **Resolve imports.** Language-aware resolvers cover relative paths, `@/` aliases, npm workspaces, Python packages, Go modules, Rust `mod`/`use`/crates, Java and PHP namespaces, C/C++ includes and Ruby `require`.
4. **Resolve calls.** A call binds to a definition in the same file first, then to one in the files it imports, then by package or directory, and finally to a unique global name. Member calls such as `obj.get()` never bind on a generic name alone, and `this.`/`self.` calls stay inside the owning type. Ambiguous calls are left unresolved rather than guessed.
5. **Rank and cluster.** Weighted PageRank over the file graph finds central files, and symbols are ranked by who calls them. Louvain modularity finds modules, which are named after the directory that holds most of their central files.
6. **Chunk and index.** Every function, method and class becomes a BM25 chunk, and large classes are split into their members. The rest of each file is split into fixed windows. Tokens split camelCase and snake_case. Symbol names are boosted, and path terms count too.
7. **Serve.** The MCP server holds the index in memory. It checks a cheap file fingerprint before each query and rebuilds only when something changed.

## Accuracy notes

repomap resolves calls statically. It does not see dynamic dispatch, reflection, dependency injection or string-based routing, and when a call could mean several definitions it leaves the call out rather than guess. `impact` therefore also lists **importing files**, which catches usage that the call graph misses. Treat call edges as strong hints with a `file:line` you can check, not as proof.
