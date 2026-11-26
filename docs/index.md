---
layout: home
title: repomap
titleTemplate: ":title — codebase map, search and change impact for AI agents"
hero:
  name: repomap
  text: Give your agent the map before it edits.
  tagline: "A map of your codebase for AI agents: code graph, search, call paths and change impact. No API key."
  image:
    src: /logo.svg
    alt: ""
  actions:
    - theme: brand
      text: Get started
      link: /guide/quickstart
    - theme: alt
      text: Live demo
      link: /demo
    - theme: alt
      text: GitHub
      link: https://github.com/SylphxAI/repomap
install: npx -y @sylphx/repomap setup
proof:
  - value: "83.0%"
    label: SWE-bench Verified issues with every fix file in one search's top 10 (semble 71.0%)
    link: /benchmarks#file-localization
  - value: "0.846"
    label: NDCG@10 on Django, Kubernetes, VS Code and rust-analyzer (semble 0.799)
    link: /benchmarks#search-quality
  - value: Live map
    label: Explore Excalidraw’s modules and find files or symbols
    link: /demo
media:
  kind: video
  src: /img/demo.mp4
  poster: /img/hero-excalidraw.webp
  alt: "repomap demo: the map of excalidraw, search, a symbol's code and callers, then the impact of changing it"
  caption: "Excalidraw walkthrough: find a symbol, inspect its code and callers, then see what depends on its file."
  link: /demo
  linkText: Try it live →
features:
  - title: map
    details: Modules found from real dependencies, the most central files (PageRank), key symbols and entry points. Zoom into any directory for an outline with line numbers.
  - title: search
    details: Symbol names, BM25 and a local code embedding model over whole functions, methods and classes. Ask in plain words or by name. Returns file:line ranges and the matching lines.
  - title: context
    details: One call for a symbol's code, its callers with call sites, callees, subtypes, members and the tests that reach it.
  - title: trace
    details: The shortest call path between two symbols, each hop cited file:line, or the call tree above or below one.
  - title: impact
    details: Blast radius before you edit, or of your current git diff. Callers by depth, importing files, modules touched, tests to run, risk level.
  - title: Fast and local
    details: Parallel Rust indexer with a per-file cache and an in-memory graph that refreshes as you edit. Nothing leaves your machine.
---

<!-- localization-hero:start -->
<p>One issue-text search finds every fix file in the top 10 for <strong>83.0% of SWE-bench Verified issues</strong> (semble: 71.0%). File retrieval, not issue resolution.</p>
<svg class="localization-chart" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 720 548" role="img" aria-labelledby="localization-title localization-desc" style="width:100%;max-width:720px;height:auto">
<title id="localization-title">SWE-bench Verified file localization</title>
<desc id="localization-desc">500 issues, one issue-text search each. Acc@k counts issues where every gold fix file is among the top k files. repomap: Acc@1 48.8%, Acc@5 74.8%, Acc@10 83.0%. semble: Acc@1 34.0%, Acc@5 61.6%, Acc@10 71.0%. BM25: Acc@1 20.8%, Acc@5 45.4%, Acc@10 55.8%. Higher is better. This measures file retrieval, not issue resolution.</desc>

<defs><pattern id="texture-45" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><rect width="6" height="6" fill="var(--surface)"/><path d="M0 0V6" stroke="var(--ink)" stroke-width="2"/></pattern><pattern id="texture-135" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(135)"><rect width="6" height="6" fill="var(--surface)"/><path d="M0 0V6" stroke="var(--ink)" stroke-width="2"/></pattern></defs>
<rect width="720" height="548" rx="16" fill="var(--surface)"/>
<text x="40" y="46" font-size="27" font-weight="600">Find the files behind an issue</text>
<text class="secondary" x="40" y="75">SWE-bench Verified · 500 issues · one search</text>
<rect class="repomap" x="40" y="92" width="16" height="16" rx="2"/><text x="66" y="107">repomap</text>
<rect class="semble" x="230" y="92" width="16" height="16" rx="2"/><text x="256" y="107">semble</text>
<rect class="bm25" x="420" y="92" width="16" height="16" rx="2"/><text x="446" y="107">BM25</text>
<path class="grid" d="M168 124V454"/><text class="secondary tick" x="168" y="482" text-anchor="middle">0</text>
<path class="grid" d="M283 124V454"/><text class="secondary tick" x="283" y="482" text-anchor="middle">25</text>
<path class="grid" d="M398 124V454"/><text class="secondary tick" x="398" y="482" text-anchor="middle">50</text>
<path class="grid" d="M513 124V454"/><text class="secondary tick" x="513" y="482" text-anchor="middle">75</text>
<path class="grid" d="M628 124V454"/><text class="secondary tick" x="628" y="482" text-anchor="middle">100</text>
<text class="group" x="40" y="138">Acc@1</text>
<g class="bar" tabindex="0" role="img" aria-label="repomap Acc@1: 48.8%">
<title>repomap Acc@1: 48.8%</title><rect x="168" y="147" width="460" height="27" fill="transparent"/>
<text x="148" y="166" text-anchor="end">repomap</text>
<path class="repomap" d="M168 152h220.47999999999996q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="value" x="402.47999999999996" y="166">48.8%</text></g>
<g class="bar" tabindex="0" role="img" aria-label="semble Acc@1: 34.0%">
<title>semble Acc@1: 34.0%</title><rect x="168" y="174" width="460" height="27" fill="transparent"/>
<text x="148" y="193" text-anchor="end">semble</text>
<path class="semble" d="M168 179h152.39999999999998q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="tooltip" x="334.4" y="193">34.0%</text></g>
<g class="bar" tabindex="0" role="img" aria-label="BM25 Acc@1: 20.8%">
<title>BM25 Acc@1: 20.8%</title><rect x="168" y="201" width="460" height="27" fill="transparent"/>
<text x="148" y="220" text-anchor="end">BM25</text>
<path class="bm25" d="M168 206h91.67999999999999q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="tooltip" x="273.68" y="220">20.8%</text></g>
<text class="group" x="40" y="248">Acc@5</text>
<g class="bar" tabindex="0" role="img" aria-label="repomap Acc@5: 74.8%">
<title>repomap Acc@5: 74.8%</title><rect x="168" y="257" width="460" height="27" fill="transparent"/>
<text x="148" y="276" text-anchor="end">repomap</text>
<path class="repomap" d="M168 262h340.08q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="value" x="522.0799999999999" y="276">74.8%</text></g>
<g class="bar" tabindex="0" role="img" aria-label="semble Acc@5: 61.6%">
<title>semble Acc@5: 61.6%</title><rect x="168" y="284" width="460" height="27" fill="transparent"/>
<text x="148" y="303" text-anchor="end">semble</text>
<path class="semble" d="M168 289h279.35999999999996q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="tooltip" x="461.35999999999996" y="303">61.6%</text></g>
<g class="bar" tabindex="0" role="img" aria-label="BM25 Acc@5: 45.4%">
<title>BM25 Acc@5: 45.4%</title><rect x="168" y="311" width="460" height="27" fill="transparent"/>
<text x="148" y="330" text-anchor="end">BM25</text>
<path class="bm25" d="M168 316h204.83999999999997q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="tooltip" x="386.84" y="330">45.4%</text></g>
<text class="group" x="40" y="358">Acc@10</text>
<g class="bar" tabindex="0" role="img" aria-label="repomap Acc@10: 83.0%">
<title>repomap Acc@10: 83.0%</title><rect x="168" y="367" width="460" height="27" fill="transparent"/>
<text x="148" y="386" text-anchor="end">repomap</text>
<path class="repomap" d="M168 372h377.79999999999995q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="value" x="559.8" y="386">83.0%</text></g>
<g class="bar" tabindex="0" role="img" aria-label="semble Acc@10: 71.0%">
<title>semble Acc@10: 71.0%</title><rect x="168" y="394" width="460" height="27" fill="transparent"/>
<text x="148" y="413" text-anchor="end">semble</text>
<path class="semble" d="M168 399h322.59999999999997q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="tooltip" x="504.59999999999997" y="413">71.0%</text></g>
<g class="bar" tabindex="0" role="img" aria-label="BM25 Acc@10: 55.8%">
<title>BM25 Acc@10: 55.8%</title><rect x="168" y="421" width="460" height="27" fill="transparent"/>
<text x="148" y="440" text-anchor="end">BM25</text>
<path class="bm25" d="M168 426h252.67999999999995q4 0 4 4v8q0 4 -4 4H168Z"/>
<text class="tooltip" x="434.67999999999995" y="440">55.8%</text></g>
<text class="secondary" x="168" y="512" font-size="17">Issues with every fix file in the top k (%)</text>
</svg>

<details><summary>Chart values and evaluation protocol</summary>
<table><caption>SWE-bench Verified: every gold fix file in the top k files</caption><thead><tr><th scope="col">Method</th><th scope="col">Acc@1</th><th scope="col">Acc@5</th><th scope="col">Acc@10</th></tr></thead><tbody><tr><th scope="row">repomap</th><td>48.8%</td><td>74.8%</td><td>83.0%</td></tr>
<tr><th scope="row">semble</th><td>34.0%</td><td>61.6%</td><td>71.0%</td></tr>
<tr><th scope="row">BM25</th><td>20.8%</td><td>45.4%</td><td>55.8%</td></tr></tbody></table>
<p>Defaults selected on 300 disjoint SWE-bench test instances, excluding every Verified ID, then frozen before one evaluation on all 500 Verified instances. One search per issue, at its base commit; gold files are used only for scoring. All three methods scored 500/500, with zero errors.</p>
<p><a href="/repomap/benchmarks#file-localization">Split definitions, pinned revisions and full results</a> · <a href="https://github.com/SylphxAI/repomap/blob/main/bench/localization/results.json">Committed chart data</a></p>
</details>
<p>Trade-off: on semble's public code-search set, NDCG@10 is <strong>0.845</strong> versus semble's <strong>0.851</strong>; semble wins there. This is not a claim of better search on every task. <a href="/repomap/benchmarks#search-quality">Measured search quality and limitations</a>.</p>
<!-- localization-hero:end -->
