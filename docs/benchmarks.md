# Benchmarks

## Search quality

<!-- SEARCH:START -->
__SEARCH__
<!-- SEARCH:END -->

### Method

- **Question sets.**
  - [semble's benchmark](https://github.com/MinishLab/semble/tree/24497845460960db1839c8485319df189a889225/benchmarks), at commit `2449784`: 1,251 questions over 63 open-source repositories in 19 languages, each pinned to a commit. The questions are sorted into architecture ("how are routes registered"), semantic ("session management") and symbol (`Blueprint`). semble's authors wrote the questions and relevant files with Claude Sonnet 4.6 and checked them with an LLM judge. We use the set as published.
  - Our [large-repository set](https://github.com/SylphxAI/repomap/tree/main/bench/search): 60 plain-language questions over Django, Kubernetes, VS Code and rust-analyzer, at the commits of the speed benchmark below. We wrote the questions and answers before running any search, and checked every answer path against the pinned tree. Because we wrote this set, read it together with the public one.
- **Scoring**, the same as semble's `run_benchmark.py`:
  - Take the top 10 results of the MCP `search` tool.
  - A target file counts as found at the rank of the first result in that file whose lines overlap the target's lines, if the target gives any.
  - NDCG@10 is averaged per repository, then per language, then over the languages.
- **Runs.** On one GitHub-hosted `ubuntu-latest` runner in the [`bench` workflow](https://github.com/SylphxAI/repomap/actions/workflows/bench.yml):
  - this build;
  - this build with `REPOMAP_EMBED=0` (keywords only);
  - repomap 1.2.2, the previous release;
  - semble itself, through its own harness, on the same repositories.
- **Speed columns.**
  - repomap's index time is `repomap index --no-cache` as a new process, which includes loading the model.
  - repomap's query time is a round trip over MCP stdio.
  - semble's numbers come from its harness: indexing in-process, and a Python function call per query.
  - So the speed columns show the size of the costs. They are not a race.
- **Tuning.** The ranking weights were tuned on 20 of the 63 repositories (one or two per language), then fixed. The table covers all 63.

## Indexing speed

Measured by [`scripts/bench.py`](https://github.com/SylphxAI/repomap/blob/main/scripts/bench.py) in the [`bench` workflow](https://github.com/SylphxAI/repomap/actions/workflows/bench.yml) on a standard GitHub-hosted `ubuntu-latest` runner. Anyone can re-run it.

<!-- BENCH:START -->
Run [36094170937](https://github.com/SylphxAI/repomap/actions/runs/36094170937) (repomap 1.0.0, 4 vCPU `ubuntu-latest`):

| Repository | Code files | Symbols | Resolved calls | Cold index | Warm index | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|
| kubernetes (Go) | 11,710 | 101,176 | 200,157 | 11.2 s | 1.88 s | 456 MB |
| vscode (TypeScript) | 6,125 | 95,785 | 182,506 | 8.0 s | 1.35 s | 320 MB |
| django (Python) | 2,271 | 41,019 | 59,643 | 2.2 s | 0.42 s | 125 MB |
| rust-analyzer (Rust) | 1,512 | 29,024 | 59,438 | 1.8 s | 0.31 s | 101 MB |

Query latency over MCP stdio, p50 / p95:

| Repository | search | context | impact | trace |
|---|---:|---:|---:|---:|
| kubernetes | 57.3 / 68.2 ms | 0.8 / 4.2 ms | 5.3 / 9.8 ms | 0.4 / 0.5 ms |
| vscode | 6.6 / 47.6 ms | 1.6 / 2.2 ms | 6.4 / 9.0 ms | 0.3 / 0.3 ms |
| django | 20.9 / 26.0 ms | 0.3 / 1.9 ms | 0.8 / 3.2 ms | 0.2 / 0.3 ms |
| rust-analyzer | 3.6 / 17.3 ms | 0.4 / 1.1 ms | 1.7 / 2.9 ms | 0.3 / 0.3 ms |
<!-- BENCH:END -->

## Method

- **Corpora:** shallow clones at fixed tags: microsoft/vscode `1.104.0` (TypeScript), rust-lang/rust-analyzer `2026-09-21` (Rust), django/django `5.2.6` (Python) and kubernetes/kubernetes `v1.34.1` (Go).
- **Cold index:** `repomap index <repo> --no-cache`, the median of 3 runs. This covers the walk, tree-sitter parse, import and call resolution, PageRank, Louvain and BM25.
- **Warm index:** the same command with the per-file cache populated. This is what a restart of the MCP server costs.
- **Peak RSS:** the highest resident memory of the index runs.
- **Query latency:** one long-lived `repomap mcp` process, with the round trip measured over stdio JSON-RPC. `search` runs 5 phrases plus the 10 most used symbol names. `context`, `impact` and `trace` (callers) each run on those 10 symbols. The table shows p50; p95 is in the JSON artifact.

## Versus GitNexus

We have not run GitNexus in this benchmark. Its PolyForm Noncommercial licence does not allow use for a company's commercial purposes, and a vendor benchmark could count as one. Its README documents its own performance. You are welcome to run both tools on the same corpora on your own machine.
