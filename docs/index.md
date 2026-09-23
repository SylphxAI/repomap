---
layout: home
hero:
  name: Spine
  text: Repository architecture with file-level proof.
  tagline: A local graph of the repo. Paths, traces, and impact cite the file and the line. No model call.
  actions:
    - theme: brand
      text: Get started
      link: /guide/quickstart
    - theme: alt
      text: Star on GitHub
      link: https://github.com/SylphxAI/spine
---

<div class="sp-section">
  <span class="sp-eyebrow">The difference</span>
  <h2 class="sp-h2">A file list is not a map.<br />A line you can open is.</h2>
  <p class="sp-lead">Spine answers where a behavior lives, which call reaches it, and what else depends on it. The default graph is regex extraction, not a language server and not tree-sitter. Synth AST stays off unless you set <code>ARCHITECTURE_READER_USE_SYNTH=1</code>.</p>
  <div class="sp-compare" style="margin-top:28px">
    <div class="side">
      <h3>What a guess says</h3>
      <p>“authMiddleware probably calls the token helper.”</p>
    </div>
    <div class="side good">
      <h3>What Spine returns</h3>
      <p><span class="sp-cite">src/auth/middleware.ts:4</span> defines authMiddleware · one <span class="sp-cite">calls</span> hop to validateToken · provenance extracted · the call is line 7.</p>
    </div>
  </div>
</div>

## One search. The line included.

```json
{
  "status": "ok",
  "tool": "architecture_search",
  "route": { "engine": "rust-core", "path": "architecture_search" },
  "answer": {
    "matches": [{
      "label": "authMiddleware",
      "kind": "symbol",
      "path": "src/auth/middleware.ts",
      "scoreExplain": ["exact_label", "kind=symbol", "kindBoost=0.4"]
    }]
  },
  "evidence": [{
    "id": "ev_64",
    "kind": "ast",
    "path": "src/auth/middleware.ts",
    "startLine": 4,
    "endLine": 4,
    "extractor": "call-graph@0.1.0",
    "confidence": "deterministic"
  }]
}
```

<p class="sp-fine">Excerpt of a real <code>architecture_search</code> for <code>authMiddleware</code> against <code>fixtures/sample-repo</code> (repository block omitted). Line 4 is <code>export function authMiddleware</code>. The same index records one <code>calls</code> hop to <code>validateToken</code>, provenance <code>extracted</code>, with the call on line 7 and the callee on <code>src/auth/token.ts:1</code>. While Synth is off, the result also carries the gap <code>importGraphRoute=regex_fallback</code>.</p>

<div class="sp-section">
  <span class="sp-eyebrow">How it works</span>
  <h2 class="sp-h2">Three steps from a repo to a citation</h2>
  <div class="sp-steps" style="margin-top:26px">
    <div class="sp-step">
      <div class="n">Step 1</div>
      <h3>Add it to your agent</h3>
      <p>One <code>npx</code> line. A stdio MCP server starts for Claude, Cursor, VS Code, Codex, or any other MCP client. No API key.</p>
    </div>
    <div class="sp-step">
      <div class="n">Step 2</div>
      <h3>Refresh, then ask</h3>
      <p><code>architecture_index</code> with no mode reuses the index, updates changed files, or scans, and names which one happened. <code>architecture_search</code>, <code>architecture_path</code>, <code>architecture_trace</code>, and <code>architecture_impact</code> are separate calls.</p>
    </div>
    <div class="sp-step">
      <div class="n">Step 3</div>
      <h3>Open the line</h3>
      <p>Use the path, the line range, and the provenance. If the gap says the graph fell back to regex, do not describe it as a compiler AST.</p>
    </div>
  </div>
</div>

<div class="sp-section">
  <span class="sp-eyebrow">What you call</span>
  <h2 class="sp-h2">Name the question. Nothing else runs.</h2>
  <p class="sp-lead">Indexing is the only scan. Search, path, trace, impact, and evidence do not rebuild the repository unless you ask the index to.</p>
  <div class="sp-grid three" style="margin-top:26px">
    <div class="sp-card">
      <h3>architecture_index</h3>
      <p>Default <code>refresh</code>: cache hit, incremental, or full. <code>full</code> is a named rescan. <code>status_only</code> does not index.</p>
    </div>
    <div class="sp-card">
      <h3>architecture_search</h3>
      <p>Ranked symbols, routes, and paths. Each match keeps a file and a line.</p>
    </div>
    <div class="sp-card">
      <h3>architecture_path</h3>
      <p>Hops between two nodes. Provenance is <code>extracted</code> or <code>inferred</code>.</p>
    </div>
    <div class="sp-card">
      <h3>architecture_trace</h3>
      <p>Follow imports or calls out from one node, with the same evidence.</p>
    </div>
    <div class="sp-card">
      <h3>architecture_impact</h3>
      <p>Incoming dependents and outgoing dependencies for a file you are about to edit.</p>
    </div>
    <div class="sp-card">
      <h3>architecture_evidence</h3>
      <p>The file and line behind a node you already have. A missing node stays a gap.</p>
    </div>
  </div>
</div>
