# @sylphx/locus

**Locus is now [repomap](https://github.com/SylphxAI/repomap)** — a map of your codebase for AI agents: code graph, hybrid search, call paths and change impact, with an interactive graph UI.

This package is a thin alias that installs `@sylphx/repomap` and runs it, so existing configs keep working. New installs should use:

```bash
npx -y @sylphx/repomap setup
```

Old tool names (`architecture_*`, `codebase_search`) are still accepted by the server until repomap 2.0.
