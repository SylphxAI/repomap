---
title: Live demo
---

# Live demo

These maps were made with `repomap export` from well-known open-source repositories. Each one is a single HTML file, rebuilt by CI whenever repomap changes. Nothing is installed; everything runs in your browser.

<div class="demo-grid">
  <a class="demo-card" href="/repomap/demo/excalidraw/" target="_blank"><b>excalidraw</b><span>TypeScript · 687 files · virtual whiteboard</span></a>
  <a class="demo-card" href="/repomap/demo/axum/" target="_blank"><b>axum</b><span>Rust · web framework on tokio</span></a>
  <a class="demo-card" href="/repomap/demo/flask/" target="_blank"><b>flask</b><span>Python · the micro web framework</span></a>
</div>

<iframe class="demo-frame" src="/repomap/demo/excalidraw/" title="repomap: excalidraw" loading="lazy"></iframe>

**Try:** press <kbd>/</kbd> and type `restoreElements`, then press <kbd>Enter</kbd>. Click **Impact** (or press <kbd>i</kbd>) to see everything that depends on the file, and click a module in the legend to focus it. The **Open ↗** links go to the exact commit on GitHub.

For your own repository, the live version adds code search and code preview:

```bash
npx -y @sylphx/repomap serve
```
