#!/usr/bin/env python3
"""repomap benchmark: index time (cold / warm), peak memory, and MCP query latency.

usage: bench.py <repomap-binary> <corpus-dir> <out.json>
Each subdirectory of <corpus-dir> is one repository.
"""
import json, os, resource, statistics, subprocess, sys, tempfile, time

BIN, CORPUS, OUT = sys.argv[1], sys.argv[2], sys.argv[3]
PHRASES = ["parse configuration file", "http request handler", "error handling retry", "cache invalidation", "authentication token"]


def index(repo, cache_dir, no_cache):
    args = [BIN, "index", repo, "--json"] + (["--no-cache"] if no_cache else [])
    env = dict(os.environ, REPOMAP_CACHE_DIR=cache_dir)
    t = time.perf_counter()
    p = subprocess.run(args, capture_output=True, text=True, env=env, check=True)
    wall = (time.perf_counter() - t) * 1000
    # Max RSS over child processes so far (KiB on Linux); cold runs come first.
    rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    return json.loads(p.stdout), wall, rss / 1024


class Mcp:
    def __init__(self, repo, cache_dir):
        env = dict(os.environ, REPOMAP_CACHE_DIR=cache_dir)
        self.p = subprocess.Popen([BIN, "mcp", "--root", repo], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        self.id = 0
        self.call_raw("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "bench", "version": "1"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def call_raw(self, method, params):
        self.id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        while True:
            m = json.loads(self.p.stdout.readline())
            if m.get("id") == self.id:
                return m

    def tool(self, name, args):
        t = time.perf_counter()
        r = self.call_raw("tools/call", {"name": name, "arguments": args})
        return (time.perf_counter() - t) * 1000, r["result"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def pct(xs, q):
    xs = sorted(xs)
    return round(xs[min(len(xs) - 1, int(len(xs) * q))], 1)


results = []
for name in sorted(os.listdir(CORPUS)):
    repo = os.path.join(CORPUS, name)
    if not os.path.isdir(repo):
        continue
    cache = tempfile.mkdtemp()
    colds = []
    for _ in range(3):
        stats, wall, rss = index(repo, cache, True)
        colds.append((wall, rss))
    _, _, _ = index(repo, cache, False)  # populate cache
    warm_stats, warm_wall, warm_rss = index(repo, cache, False)
    mcp = Mcp(repo, cache)
    first_ms, _ = mcp.tool("map", {})
    mp = json.loads(mcp.tool("map", {"format": "json", "limit": 20})[1]["content"][0]["text"])
    syms = [s["symbol"]["name"] for s in mp["key_symbols"]][:10]
    lat = {"search": [], "context": [], "impact": [], "trace": []}
    for q in PHRASES + [s.split(".")[-1] for s in syms]:
        lat["search"].append(mcp.tool("search", {"query": q})[0])
    for s in syms:
        lat["context"].append(mcp.tool("context", {"target": s})[0])
        lat["impact"].append(mcp.tool("impact", {"target": s})[0])
        lat["trace"].append(mcp.tool("trace", {"from": s, "direction": "callers"})[0])
    mcp.close()
    row = {
        "repo": name,
        "files": stats["code_files"],
        "symbols": stats["symbols"],
        "call_edges": stats["call_edges"],
        "cold_index_ms": round(statistics.median(w for w, _ in colds)),
        "warm_index_ms": round(warm_wall),
        "peak_rss_mb": round(max(r for _, r in colds)),
        "mcp_first_call_ms": round(first_ms),
        **{f"{k}_p50_ms": pct(v, 0.5) for k, v in lat.items()},
        **{f"{k}_p95_ms": pct(v, 0.95) for k, v in lat.items()},
    }
    results.append(row)

cpu = subprocess.run(["nproc"], capture_output=True, text=True).stdout.strip()
json.dump({"cpus": cpu, "results": results}, open(OUT, "w"), indent=2)
print(f"### repomap benchmark ({cpu} vCPU GitHub-hosted runner)\n")
print("| repo | code files | symbols | call edges | cold index | warm index | peak RSS | search p50 | context p50 | impact p50 | trace p50 |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
for r in results:
    print(f"| {r['repo']} | {r['files']:,} | {r['symbols']:,} | {r['call_edges']:,} | {r['cold_index_ms']/1000:.1f} s | {r['warm_index_ms']/1000:.2f} s | {r['peak_rss_mb']} MB | {r['search_p50_ms']} ms | {r['context_p50_ms']} ms | {r['impact_p50_ms']} ms | {r['trace_p50_ms']} ms |")
