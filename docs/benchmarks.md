# Benchmarks

## Search quality

<!-- SEARCH:START -->
Run [36204314484](https://github.com/SylphxAI/repomap/actions/runs/36204314484), 4 vCPU `ubuntu-latest`. NDCG@10, where higher is better and 1.0 means every relevant file is at the top.

**semble's public benchmark**: 63 repositories, 19 languages, 1,251 questions.

| Method | NDCG@10 | architecture | semantic | symbol | index, mean | query p50 |
|---|---:|---:|---:|---:|---:|---:|
| **repomap 1.3** | **0.851** | 0.806 | 0.859 | 0.924 | 152 ms | 2.2 ms |
| repomap 1.3, keywords only (`REPOMAP_EMBED=0`) | 0.810 | 0.746 | 0.818 | 0.922 | 121 ms | 1.7 ms |
| repomap 1.2.2 | 0.685 | 0.614 | 0.668 | 0.883 | 116 ms | 1.6 ms |
| semble, same runner | 0.851 | | | | 1,352 ms | 4.1 ms |

semble's README also lists results from its authors' machine: CodeRankEmbed (a 137M-parameter transformer) 0.839, ColGREP 0.693, BM25 0.673, codebase-memory-mcp 0.630, ripgrep 0.126.

**Large repositories**: 60 questions of our own over Django, Kubernetes, VS Code and rust-analyzer.

| Method | mean | django | kubernetes | rust-analyzer | vscode | index, mean |
|---|---:|---:|---:|---:|---:|---:|
| **repomap 1.3** | **0.794** | 0.846 | 0.924 | 0.745 | 0.659 | 4.9 s |
| repomap 1.3, keywords only | 0.736 | 0.857 | 0.740 | 0.717 | 0.629 | 4.2 s |
| repomap 1.2.2 | 0.463 | 0.486 | 0.567 | 0.386 | 0.412 | 4.0 s |
| semble, same runner | 0.799 | 0.829 | 0.824 | 0.712 | 0.831 | 51.5 s |

What this shows:
- On the public set, repomap 1.3 ties semble, which is built only for search, and is above every other tool semble lists, CodeRankEmbed included.
- On large repositories the two are close. semble is clearly better on VS Code, and repomap on Kubernetes.
- The embeddings add 0.04 on the public set and 0.06 on large repositories.
- Most of the gain over 1.2.2 comes from the new ranking, which also helps with embeddings off.
- Weakest language: TypeScript. Details are in the table below.

<details>
<summary>By language (repomap 1.3, public set)</summary>

| Language | NDCG@10 |
|---|---:|
| bash | 0.896 |
| c | 0.784 |
| cpp | 0.881 |
| csharp | 0.879 |
| elixir | 0.910 |
| go | 0.881 |
| haskell | 0.797 |
| java | 0.811 |
| javascript | 0.896 |
| kotlin | 0.824 |
| lua | 0.880 |
| php | 0.830 |
| python | 0.882 |
| ruby | 0.828 |
| rust | 0.840 |
| scala | 0.907 |
| swift | 0.817 |
| typescript | 0.724 |
| zig | 0.902 |

</details>
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
Run [36202944872](https://github.com/SylphxAI/repomap/actions/runs/36202944872) (repomap 1.3.0, 4 vCPU `ubuntu-latest`), with embeddings and with `REPOMAP_EMBED=0`:

| Repository | Code files | Symbols | Resolved calls | Cold index | Cold, keywords only | Warm index | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| kubernetes (Go) | 11,710 | 101,176 | 200,256 | 13.0 s | 10.9 s | 1.92 s | 572 MB |
| vscode (TypeScript) | 6,126 | 95,786 | 182,506 | 9.2 s | 7.9 s | 1.41 s | 408 MB |
| django (Python) | 2,271 | 41,019 | 59,643 | 2.6 s | 2.2 s | 0.45 s | 173 MB |
| rust-analyzer (Rust) | 1,512 | 29,024 | 59,438 | 2.1 s | 1.8 s | 0.36 s | 152 MB |

Embeddings add 16–19% to a cold index, 5–8% to a warm one, and about 30% to peak memory.

Query latency over MCP stdio, p50 / p95:

| Repository | search | context | impact | trace |
|---|---:|---:|---:|---:|
| kubernetes | 72.8 / 88.1 ms | 1.0 / 4.4 ms | 6.1 / 11.7 ms | 0.6 / 0.8 ms |
| vscode | 16.3 / 55.8 ms | 1.8 / 2.6 ms | 7.7 / 10.7 ms | 0.5 / 0.7 ms |
| django | 24.3 / 29.4 ms | 0.4 / 2.3 ms | 0.9 / 4.1 ms | 0.3 / 0.4 ms |
| rust-analyzer | 7.3 / 21.5 ms | 0.6 / 1.2 ms | 2.0 / 3.4 ms | 0.4 / 0.5 ms |
<!-- BENCH:END -->

## Method

- **Corpora:** shallow clones at fixed tags: microsoft/vscode `1.104.0` (TypeScript), rust-lang/rust-analyzer `2026-09-21` (Rust), django/django `5.2.6` (Python) and kubernetes/kubernetes `v1.34.1` (Go).
- **Cold index:** `repomap index <repo> --no-cache`, the median of 3 runs. This covers the walk, tree-sitter parse, embeddings, import and call resolution, PageRank, Louvain and BM25.
- **Warm index:** the same command with the per-file cache populated. This is what a restart of the MCP server costs.
- **Peak RSS:** the highest resident memory of the index runs.
- **Query latency:** one long-lived `repomap mcp` process, with the round trip measured over stdio JSON-RPC. `search` runs 5 phrases plus the 10 most used symbol names. `context`, `impact` and `trace` (callers) each run on those 10 symbols. The table shows p50; p95 is in the JSON artifact.

## Versus GitNexus

We have not run GitNexus in this benchmark. Its PolyForm Noncommercial licence does not allow use for a company's commercial purposes, and a vendor benchmark could count as one. Its README documents its own performance. You are welcome to run both tools on the same corpora on your own machine.
