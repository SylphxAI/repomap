#!/usr/bin/env python3
"""Search quality on public benchmarks, scored like the benchmark's own harness.

usage: bench_search.py <repomap-binary> <suite> <corpus-dir> <out.json> [--repo NAME ...]
       bench_search.py --summary <dir>    # Markdown tables from the JSON files in <dir>

<suite> is a folder with repos.json and annotations/<repo>.json, in the format
of semble's benchmark (github.com/MinishLab/semble, benchmarks/): each task has
a query, `relevant` and `secondary` targets (a path, or a path with a line
span) and a category. Our own large-repo set in bench/search uses the same
format.

Scoring is the same as semble's run_benchmark.py: the top 10 results of the
MCP `search` tool, a target counts as found at the rank of the first result
whose file matches and whose lines overlap the target's span, and NDCG@10 is
averaged per repo, then per language, then over languages.

Each repo is cloned at its pinned revision into <corpus-dir>. Set
REPOMAP_EMBED=0 to measure keyword-only search.
"""
import json
import math
import os
import statistics
import subprocess
import sys
import tempfile
import time
from collections import defaultdict

K = 10
if sys.argv[1] != "--summary":
    BIN, SUITE, CORPUS, OUT = sys.argv[1:5]
    ONLY = [a for i, a in enumerate(sys.argv) if i > 0 and sys.argv[i - 1] == "--repo"]


def clone(spec):
    dest = os.path.join(CORPUS, spec["name"])
    if os.path.isdir(os.path.join(dest, ".git")):
        return dest
    os.makedirs(dest, exist_ok=True)
    run = lambda *a: subprocess.run(["git", "-C", dest, *a], check=True, capture_output=True)
    run("init", "-q")
    run("remote", "add", "origin", spec["url"])
    run("fetch", "-q", "--depth", "1", "origin", spec["revision"])
    run("checkout", "-q", "FETCH_HEAD")
    return dest


class Mcp:
    def __init__(self, root, env):
        self.p = subprocess.Popen([BIN, "mcp", "--root", root], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        self.id = 0
        self.call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "bench", "version": "1"}})
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, m):
        self.p.stdin.write(json.dumps(m) + "\n")
        self.p.stdin.flush()

    def call(self, method, params):
        self.id += 1
        self.send({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params})
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("id") == self.id:
                return m

    def search(self, query):
        t = time.perf_counter()
        r = self.call("tools/call", {"name": "search", "arguments": {"query": query, "limit": K, "format": "json"}})
        ms = (time.perf_counter() - t) * 1000
        res = r["result"]
        if res.get("isError"):
            return ms, []
        return ms, json.loads(res["content"][0]["text"])["hits"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=30)


def path_matches(a, b):
    return a == b or a.endswith("/" + b) or b.endswith("/" + a)


def rank_of(hits, target):
    path = target if isinstance(target, str) else target["path"]
    span = None if isinstance(target, str) or target.get("start_line") is None else (int(target["start_line"]), int(target["end_line"]))
    for i, h in enumerate(hits, 1):
        if path_matches(h["file"], path) and (span is None or not (h["end"] < span[0] or h["start"] > span[1])):
            return i
    return None


def ndcg(ranks, n_relevant):
    if n_relevant == 0:
        return 0.0
    dcg = sum(1 / math.log2(r + 1) for r in ranks if 1 <= r <= K)
    ideal = sum(1 / math.log2(i + 2) for i in range(min(K, n_relevant)))
    return dcg / ideal


def index_ms(root, cache):
    env = dict(os.environ, REPOMAP_CACHE_DIR=cache)
    t = time.perf_counter()
    out = subprocess.run([BIN, "index", root, "--no-cache", "--json"], check=True, capture_output=True, text=True, env=env).stdout
    return (time.perf_counter() - t) * 1000, json.loads(out)


def main():
    specs = json.load(open(os.path.join(SUITE, "repos.json")))
    os.makedirs(CORPUS, exist_ok=True)
    os.makedirs(os.path.dirname(os.path.abspath(OUT)), exist_ok=True)
    repos = []
    for spec in specs:
        ann = os.path.join(SUITE, "annotations", spec["name"] + ".json")
        if (ONLY and spec["name"] not in ONLY) or not os.path.exists(ann):
            continue
        root = clone(spec)
        if spec.get("benchmark_root"):
            root = os.path.join(root, spec["benchmark_root"])
        cache = tempfile.mkdtemp(prefix="repomap-bench-")
        ms_index, stats = index_ms(root, cache)
        mcp = Mcp(root, dict(os.environ, REPOMAP_CACHE_DIR=cache))
        per_cat = defaultdict(list)
        scores, lat = [], []
        for task in json.load(open(ann)):
            ms, hits = mcp.search(task["query"])
            lat.append(ms)
            targets = task["relevant"] + task.get("secondary", [])
            ranks = [r for t in targets if (r := rank_of(hits, t)) is not None]
            s = ndcg(ranks, len(targets))
            scores.append(s)
            per_cat[task.get("category", "unknown")].append(s)
        mcp.close()
        row = {
            "repo": spec["name"],
            "language": spec["language"],
            "queries": len(scores),
            "ndcg10": round(statistics.mean(scores), 4),
            "by_category": {c: round(statistics.mean(v), 4) for c, v in sorted(per_cat.items())},
            "index_ms": round(ms_index),
            "files": stats["files"],
            "chunks": stats["chunks"],
            "p50_ms": round(statistics.median(lat[1:] or lat), 2),
            "_cat": per_cat,
        }
        repos.append(row)
        print(f"{row['repo']:<24} {row['language']:<11} ndcg@10 {row['ndcg10']:.3f}  index {row['index_ms']:>6} ms  p50 {row['p50_ms']:>6} ms", file=sys.stderr, flush=True)

    by_lang = defaultdict(list)
    for r in repos:
        by_lang[r["language"]].append(r)
    languages = {l: {"repos": len(v), "ndcg10": round(statistics.mean(x["ndcg10"] for x in v), 4)} for l, v in sorted(by_lang.items())}
    cats = defaultdict(list)
    for r in repos:
        for c, v in r.pop("_cat").items():
            cats[c].extend(v)
    summary = {
        "embed": os.environ.get("REPOMAP_EMBED", "1") not in ("0", "false", "off"),
        "repos": len(repos),
        "queries": sum(r["queries"] for r in repos),
        "ndcg10": round(statistics.mean(l["ndcg10"] for l in languages.values()), 4) if languages else 0,
        "ndcg10_by_query_count": round(sum(r["ndcg10"] * r["queries"] for r in repos) / max(1, sum(r["queries"] for r in repos)), 4),
        "by_category": {c: round(statistics.mean(v), 4) for c, v in sorted(cats.items())},
        "index_ms_mean": round(statistics.mean(r["index_ms"] for r in repos)) if repos else 0,
        "p50_ms_mean": round(statistics.mean(r["p50_ms"] for r in repos), 2) if repos else 0,
    }
    json.dump({"summary": summary, "by_language": languages, "repos": repos}, open(OUT, "w"), indent=2)
    print(json.dumps(summary, indent=2))


LARGE = {"django", "kubernetes", "vscode", "rust-analyzer"}


def summarize(d):
    """Markdown tables for the runs in directory d (see bench.yml)."""
    def load(name):
        p = os.path.join(d, name + ".json")
        return json.load(open(p)) if os.path.exists(p) else None

    def lang_mean(repos):
        by = defaultdict(list)
        for r in repos:
            by[r["language"]].append(r["ndcg10"])
        return statistics.mean(statistics.mean(v) for v in by.values()) if by else float("nan")

    rows = [("repomap (this build)", "hybrid"), ("repomap, keywords only", "keywords"), ("repomap 1.2.2", "before")]
    print("### semble benchmark: 63 repositories, 19 languages, 1,251 questions (NDCG@10)\n")
    print("| Method | NDCG@10 | architecture | semantic | symbol | index (mean) | query p50 |")
    print("|---|---|---|---|---|---|---|")
    for label, key in rows:
        r = load("semble-" + key)
        if r:
            s = r["summary"]
            c = s["by_category"]
            print(f"| {label} | **{s['ndcg10']:.3f}** | {c.get('architecture', 0):.3f} | {c.get('semantic', 0):.3f} | {c.get('symbol', 0):.3f} | {s['index_ms_mean']} ms | {s['p50_ms_mean']} ms |")
    me = load("semble-self")
    if me:
        small = [r for r in me["repos"] if r["repo"] not in LARGE]
        print(f"| semble (same runner) | **{lang_mean(small):.3f}** | | | | {statistics.mean(r['index_ms'] for r in small):.0f} ms | {statistics.mean(r['p50_ms'] for r in small):.2f} ms |")
    print("\n### Large repositories: 60 questions over Django, Kubernetes, VS Code, rust-analyzer (NDCG@10)\n")
    print("| Method | mean | " + " | ".join(sorted(LARGE)) + " | index (mean) |")
    print("|---|---|" + "---|" * len(LARGE) + "---|")
    for label, key in rows:
        r = load("large-" + key)
        if r:
            per = {x["repo"]: x for x in r["repos"]}
            print(f"| {label} | **{statistics.mean(x['ndcg10'] for x in r['repos']):.3f}** | " + " | ".join(f"{per[n]['ndcg10']:.3f}" if n in per else "" for n in sorted(LARGE)) + f" | {r['summary']['index_ms_mean']} ms |")
    if me:
        per = {x["repo"]: x for x in me["repos"] if x["repo"] in LARGE}
        if per:
            print(f"| semble (same runner) | **{statistics.mean(x['ndcg10'] for x in per.values()):.3f}** | " + " | ".join(f"{per[n]['ndcg10']:.3f}" if n in per else "" for n in sorted(LARGE)) + f" | {statistics.mean(x['index_ms'] for x in per.values()):.0f} ms |")
    r = load("semble-hybrid")
    if r:
        print("\n### By language (this build, semble benchmark)\n")
        print("| Language | NDCG@10 |\n|---|---|")
        for l, v in r["by_language"].items():
            print(f"| {l} | {v['ndcg10']:.3f} |")


if sys.argv[1] == "--summary":
    summarize(sys.argv[2])
else:
    main()
