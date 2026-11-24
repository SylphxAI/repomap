# File localization on SWE-bench

For each instance, the repository is checked out at `base_commit`, and each
method gets only the issue text (at most 20,000 characters) in one search call.
We score whether the files the gold patch modifies are among the top ranked files.
Gold files and line ranges are used for scoring, never as search input.

## Splits

- `manifest.json`: all 500 [SWE-bench Verified](https://huggingface.co/datasets/princeton-nlp/SWE-bench_Verified)
  instances, pinned dataset revision `c104f840cc67f8b6eec6f759ebc8b2693d585d4a`
  (MIT). Includes gold files and changed old-file line ranges, plus a
  50-instance smoke set (every tenth by id).
- `manifest-tune.json`: 300 instances from the full SWE-bench test set, revision
  `e48e2bd1e9fecd5bbd641e9414ac59da9f2e69f6`, **excluding every Verified id**.
  Sampled evenly by sorted instance id, among instances with an existing gold
  file. This is a tuning set, not SWE-bench Lite; its smoke set has 30 instances.
- `localize.py`: `prepare` (`--split tune` for the tuning manifest), `run`
  (one shard), and `summarize` (merge shards, optionally write the Verified
  table into `docs/benchmarks.md`). Metric definitions are in its header.
- `results.json`: per-instance ranks and timings of the last full Verified run.
- `results-tune.json`: selected tuning ranks and timings, all experiment
  summaries, and per-method run provenance. The before/selected binaries use
  the same run; semble and BM25 use the earlier run on the identical split.
- `run.json`: pull-request run settings. Keep it on the tuning split so later
  documentation pushes do not measure Verified again.

Methods: repomap `search` (with and without embeddings), optional before binary
(`--baseline`, labeled `main`), BM25 over tracked text files (`rank_bm25`,
code-aware tokens), and semble pinned at `24497845460960db1839c8485319df189a889225`.
Variants share the new binary's index, but each has its own MCP process with
`REPOMAP_TUNE` set before initialization. Index timings include model loading;
query timings are a warmed MCP round trip (semble is an in-process Python call).

## Tuning and evaluation

Only `manifest-tune.json` selects ranking defaults. Two runs tested 26 single
changes and 10 combinations. Select the highest Acc@5 + Acc@10 among candidates
within 2× the before binary's median query time. Freeze the winner before the
one full Verified evaluation: `wh=1,wc=1,wf=2,qcap=48`.

Run through Actions, `bench-localization`, mode `full`, with the desired manifest.
It compiles in CI, runs 10 shards on free GitHub-hosted standard `ubuntu-latest`
runners and merges them in the summary job. The summary shows the metrics;
the raw shard JSONs are available for one day. Each shard records the frozen
manifest selection, methods and shard count.
The merger independently derives that plan from the manifest and the same
`--smoke`, `--only`, `--skip`, `--no-semble`, `--baseline`, `--variants` options;
pass `--shards N` for a multi-shard merge. Missing or duplicate shards/IDs,
unplanned or missing methods, checkout failures and method errors invalidate
the merge before it writes results or documentation. Intentionally unselected
methods/instances and BM25's inapplicable chunk metric remain excluded;
a successful empty chunk search is a miss, not an inapplicable metric.
Legacy shards without a plan cannot be merged; historical committed results
are unchanged. Run the offline regressions with
`python3 -m unittest discover -s scripts -p test_bench_validity.py`.
Before publishing numbers, verify the frozen plan matches the intended run.
Download the shard artifacts within one day and run the same local merge to
produce the result JSON in this directory. Update `docs/benchmarks.md`, the README's search claim and the CHANGELOG in
the same PR. Do not tune defaults after reading Verified.
