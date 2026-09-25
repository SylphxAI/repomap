---
layout: home
hero:
  name: repomap
  text: A map of your codebase, for you and your AI agent.
  tagline: Code graph, hybrid search, call paths and change impact, with a WebGL graph UI. One Rust binary. Local. No API key. MIT.
  actions:
    - theme: brand
      text: npx -y @sylphx/repomap setup
      link: /guide/quickstart
    - theme: alt
      text: Open the graph UI
      link: /guide/ui
    - theme: alt
      text: GitHub
      link: https://github.com/SylphxAI/repomap
features:
  - icon: 🗺️
    title: map
    details: Modules found from real dependencies, the most central files (PageRank), key symbols and entry points. Zoom into any directory for an outline with line numbers.
  - icon: 🔎
    title: search
    details: Symbol names plus BM25 over whole functions, methods and classes (tree-sitter chunks). Returns file:line ranges and the matching lines.
  - icon: 🧭
    title: context
    details: One call for a symbol's code, its callers with call sites, callees, subtypes, members and the tests that reach it.
  - icon: 🪢
    title: trace
    details: The shortest call path between two symbols, each hop cited file:line, or the call tree above or below one.
  - icon: 💥
    title: impact
    details: Blast radius before you edit, or of your current git diff. Callers by depth, importing files, modules touched, tests to run, risk level.
  - icon: ⚡
    title: Fast and local
    details: Parallel Rust indexer with a per-file cache and an in-memory graph that refreshes as you edit. Nothing leaves your machine.
---

<div class="hero-shot">
  <img src="/img/hero-excalidraw.webp" alt="repomap graph UI on the excalidraw repository">
  <p>excalidraw: 687 files, 5,001 symbols, 9,639 resolved calls, indexed in under half a second.</p>
</div>
