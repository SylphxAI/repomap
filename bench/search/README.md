# Search benchmark on large repositories

60 natural-language questions over four large repositories (Django, Kubernetes,
VS Code, rust-analyzer) at pinned commits. Each question names the file a
developer would open first; some also name a second acceptable file.

We wrote the questions and answers ourselves before looking at any search
results, and checked every answer path against the pinned tree. That makes this
set ours, not independent: read it next to the public semble benchmark
(63 repositories, 1,251 questions), which is scored the same way.

Format and scoring match `scripts/bench_search.py`. Run it with:

```bash
python3 scripts/bench_search.py target/release/repomap bench/search /tmp/corpus out.json
```
