# Agent-readiness score

```bash
repomap score                       # score, per-check detail, fixes, badge
repomap score --json
repomap score --min 70              # exit 1 below 70 (CI gate)
repomap score --update-readme README.md [--insert]
```

| Check | Points | Full marks when |
|---|---:|---|
| Agent instructions | 20 | `AGENTS.md`, `CLAUDE.md`, Copilot or Cursor instructions exist (8), are substantial (4), list build/test commands (4), and describe the layout (4) |
| Build & test commands | 15 | a test command is discoverable from the manifests (9), a build/lint entry point exists (3), and the commands are written down (3) |
| Tests | 15 | at least 0.4 test files per source file. Rust files with inline `#[test]`s count. |
| CI | 10 | a pipeline exists (6) and runs tests (4) |
| Module boundaries | 10 | most dependencies stay inside their module (repomap's Louvain modules). Small codebases get full marks. |
| File sizes | 10 | few source files over 1,000 lines |
| Docs | 10 | a README over 1,500 characters (5) with sections (2), plus ARCHITECTURE/CONTRIBUTING/docs (3) |
| Types | 10 | typed languages, strict TypeScript, or type-checked Python (weighted by lines) |

Every check that is not at full marks comes with a concrete fix, sorted by points gained. Fixes name files where possible: the largest files, the most central untested files, and the most tangled pair of modules.

## Badge

```markdown
[![agent-ready 87/100](https://mark.sylphx.com/badge/agent--ready-87%2F100-brightgreen)](https://github.com/SylphxAI/repomap#agent-readiness-score)
```

Prefer not to depend on a hosted image? `repomap score --badge-style static --update-readme README.md` writes a self-contained `.github/agent-ready.svg` and points the badge at it. The GitHub Action takes `badge-style: static`.

The colour follows the score: 85+ brightgreen, 70+ green, 55+ yellow, 40+ orange, otherwise red. `--update-readme` replaces the badge between `<!-- repomap:agent-ready -->` markers, or any existing agent-ready badge. With `--insert`, it adds the badge to the end of the README's existing badge row (for example inside a centered `<div>` header); without a badge row, it goes directly under the H1.

## GitHub Action

```yaml
name: agent-ready
on:
  push: { branches: [main] }
permissions:
  contents: write
  pull-requests: write
jobs:
  score:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: SylphxAI/repomap@v1
        with:
          update-readme: true
          min-score: 0
```

By default (`commit-mode: pr`), the Action commits the badge to one reusable branch, `repomap/agent-ready-badge`, and opens a pull request, or updates the one already open. This works with protected branches and merge queues. The repository (or organization) must allow GitHub Actions to create pull requests (Settings → Actions → General). `commit-mode: push` commits straight to the checked-out branch instead; use it only where that branch accepts direct pushes.

Inputs: `path`, `readme`, `update-readme`, `badge-style`, `commit-mode`, `pr-branch`, `token`, `commit-message`, `min-score`, `version`. Outputs: `score`, `badge-url`, `badge-markdown`. The full report goes to the job summary.
