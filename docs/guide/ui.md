# Graph UI and export

```bash
npx -y @sylphx/repomap serve [dir] [--port 7878] [--no-open]
npx -y @sylphx/repomap export [dir] [--out repomap.html]
```

<div class="shots">
  <img src="/img/impact-tokio.webp" alt="Impact view">
  <img src="/img/code-panel-tokio.webp" alt="Code panel">
</div>

**What you see.** Every node is a source file. Colours are modules, detected with Louvain community detection on the import and call graph. Node size is PageRank centrality, and edges are resolved dependencies. The layout is ForceAtlas2, running in a web worker.

| Action | How |
|---|---|
| Search files, symbols, code | <kbd>/</kbd> or <kbd>⌘K</kbd>, arrows, <kbd>Enter</kbd> |
| Inspect a file | click it: module, size, centrality, symbols, used by, depends on |
| Read a symbol | click it in the panel: source, callers and callees (serve mode) |
| Blast radius | **Impact** or <kbd>i</kbd>: dependents by depth (red → orange → yellow) |
| Dependencies | **Depends on** or <kbd>d</kbd> |
| Focus a module | click it in the legend (<kbd>Shift</kbd>-click hides it) |
| Fit / clear | <kbd>f</kbd> / <kbd>Esc</kbd> |
| Share a view | the URL hash is the selected file: `…/repomap.html#src/server.ts` |

**Export** writes one self-contained HTML file with the graph, symbol lists and UI included. It doesn't need a server. If the repo has a GitHub remote, **Open ↗** links point to the exact commit. Search in export mode covers files and symbols. Full-text code search and code preview need `serve`.
