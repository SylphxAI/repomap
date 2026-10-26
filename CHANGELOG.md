# Changelog

## 1.9.0 - 2026-10-25

- **The multi-repository workspace is free.** repomap Team is withdrawn before any licence was issued: a request that names several repository roots (`--workspace`, a `workspace` argument or a `repomap.workspace.toml`) now always gets the joined answer, with no licence and no `pro_required`. `repomap licence` only says no licence is needed. The Team page is removed.

## 1.8.0 - 2026-10-03

- **Team: every tool joins across repositories.** With a Team licence, `map` and `context` now cross the roots of a workspace like `search`, `trace` and `impact` already did, instead of quietly answering for the current repository. `map` gives one map per repository and lists which repository imports which package from which (a token budget is split across the repositories). `context` finds the target in every repository that has it and lists the files in other repositories that import it; `repo:path` asks one repository only. `db` answers once per repository and never merges schemas, because separate repositories are usually separate databases; with `url` or `url_env` it shows the one live database to each repository, so you can see which repository queries each table. Without a licence nothing changes: the current repository answers with `pro_required`.
- **`repomap licence buy`.** Opens the Team page; in-terminal purchase turns on when checkout is live. Updates `sylphx-mcp-kit` to 0.6.

## 1.7.0 - 2026-10-02

- **Ready for crates.io.** The two crates, `sylphx-repomap-core` and `sylphx-repomap` (the binary stays `repomap`), carry full package metadata, a small `include` list, and a versioned path dependency, so `cargo publish --dry-run --locked` passes in dependency order. `cargo binstall sylphx-repomap` downloads the matching GitHub release binary. Publishing stays off until an owner sets `CRATES_IO_PUBLISH_ENABLED` and the trusted-publisher records exist.
- **Security: `base` could be used to write files.** The `base` value of `impact --changed` (CLI `--base`, MCP or HTTP argument `base` or `git_base`) was passed to `git diff` as-is, so a value such as `--output=/path` made git write a file. repomap now rejects any ref that starts with `-` with a clear error, and passes `--end-of-options` before the ref so git always reads it as a revision. No other user input is passed to git or another program in a position where it could be read as an option.
- **repomap Team licence.** `repomap licence status` and `repomap licence activate <token>` show and store a Team licence (token from `REPOMAP_LICENCE_TOKEN` or the licence file), checked offline against the issuer's public key. They are free to run. A request that names more than one repository root (a `workspace` argument, `--workspace`, or a `repomap.workspace.toml` with several roots) now goes through one gate: without a Team licence it answers for the current repository, adds `pro_required` and one line saying the other repositories were not joined; with one it calls the multi-repository join. Everything free stays free, and the MCP surface stays six tools.
- New [Team page](docs/team.md) (US$180 per developer per year, packs of 5, 10, 25 and 50; public repositories free) and `docs/vision.md`, with a "Free and Team" section in the README.
- Updates `sylphx-mcp-kit` to 0.5.

## 1.6.0 - 2026-10-02

- **Token-budgeted map.** `repomap map --tokens N` (MCP `map` argument `tokens`) fills the map with the highest-ranked modules, central files and symbols until the estimated size reaches N tokens (4 characters per token, rounded up), then ends with a line such as `… 31 more files, 40 more symbols omitted (budget 1000 tokens; lowest-ranked first)`. Under a tight budget the outline and key symbols go first, then files and modules, so the structure stays readable. Without `--tokens` the output is unchanged.

- **Huge fixture trees are deferred by default.** A directory named `testdata`, `fixtures`, `__fixtures__` or `__snapshots__`, or the first directory below a `test`/`tests`/`spec`/`e2e` folder (for example `tests/cases`), with 1,000 or more files is no longer indexed up front. Folders where at least 10% of files are named like tests (`*_test.go`, `*.test.ts`, `*_spec.rb`, `*Test.java`), `src/test/**` and `src/**/test/**` layouts, and small fixture folders stay indexed. A `context`, `impact`, `trace`, `map --focus` or `search --path` that names a file or directory inside a deferred tree indexes it on demand; `--include-fixtures` (or `REPOMAP_INCLUDE_FIXTURES=1`) indexes them all. `repomap index` and `map` say how many fixture files were deferred and where. On the TypeScript compiler repository (46,459 of 47,213 files deferred: `tests/baselines`, `tests/cases`) cold index goes from 12.1 s and 610 MB to 1.2 s and 159 MB (median of 3); zod and tokio are unchanged (0.2 to 0.4 s, under 70 MB).

## 1.5.0

- **Issue-to-code search.** Long reports now contribute their title, backtick code, identifiers, stack-trace frames and named paths to ranking. Chunk BM25 and local embeddings fuse with exact symbol candidates and file-level BM25; decayed per-file aggregation spreads results across files. Lexical queries retain their path/kind filters and keywords-only mode. Defaults were frozen on a disjoint 300-instance tuning split (`wh=1,wc=1,wf=2,qcap=48`) before one full SWE-bench Verified evaluation. Acc@1/5/10 rises from 23.6/51.8/62.2 to **48.8/74.8/83.0**, ahead of semble's 34.0/61.6/71.0, with median query time 59.8 → 54.4 ms. Public search NDCG@10 trades 0.851 → 0.845 (semble still wins at 0.851); the large-repository set improves 0.794 → 0.846. [Benchmarks](docs/benchmarks.md) record both localization splits, the search trade-off and timings.

- The CLI prints one GitHub star line to stderr after the fifth successful interactive query run, once ever. It is silent for the MCP server, with `--json`, in CI, and when stderr is not a terminal; `REPOMAP_NO_STAR_HINT=1` turns it off.

## 1.4.0

- **Django models as a schema source for `db`.** `repomap db` now reads `models.py` and `models/` packages alongside the existing sources:
  - classes subclassing `models.Model` directly, through an abstract base in the repository, or through a library base (`AbstractUser`, `TimeStampedModel`); abstract and proxy models emit no table, and multi-table inheritance gives the child a `<parent>_ptr_id` primary key and only its own columns;
  - fields as columns, with `db_column`, `primary_key`, `null`, `unique` and `db_index` honoured;
  - `ForeignKey` and `OneToOneField` as foreign keys (`<attr>_id`, or the target field's column via `to_field`), resolving `"self"`, `"app.Model"` and `settings.AUTH_USER_MODEL`;
  - `ManyToManyField` as Django's implicit join table `<db_table>_<field>` (two foreign keys, a unique pair and an index) unless `through=` names an explicit model;
  - `Meta.db_table` (with `%(app_label)s` / `%(class)s` filled in), `Meta.indexes` / `Meta.constraints`, `unique_together` and `index_together`, including the backend index classes (`GinIndex`, `BTreeIndex`, …) and the ways a model reuses a base's Meta: `class Meta(Base.Meta)`, `indexes = [*Base.Meta.indexes, …]` and `indexes.extend(Base.Meta.indexes)`. The default table is `<app_label>_<modelname>`;
  - model usage (`Post.objects.filter(…)`) links back to the table from any file that imports the model's `models.py`, like the other ORM sources.

  On [saleor](https://github.com/saleor/saleor) (`repomap db --json`): 122 tables (100 models and 22 implicit M2M join tables), 245 foreign keys, 1229 columns and 508 indexes.

## 1.3.2

- `repomap score --update-readme --insert` no longer treats a bare image, such as a hero banner at the top of a centered header, as the badge row. Only linked images count as badges.

## 1.3.1

- `repomap score --update-readme --insert` adds the badge to the end of the README's existing badge row (such as the badges inside a centered `<div>` header) instead of above the whole header. With no badge row, it goes directly under the H1.
- The GitHub Action gets `commit-mode`. The default, `pr`, commits the badge to one reusable branch (`repomap/agent-ready-badge`) and opens or updates a single pull request, so it works with protected branches and merge queues. `push` keeps the old direct commit.

## 1.3.0

- **Semantic search.** `search` now also ranks code by meaning, with a local static code embedding model ([potion-code-16M-v2](https://huggingface.co/minishlab/potion-code-16M-v2), MIT, 256 dimensions). Plain questions such as "where are failed requests retried" find the right function even when it shares no word with the question.
  - The model (33 MB) downloads once from Hugging Face on first use, with a message, and is checked by SHA-256. It is shared with other Sylphx tools in `~/.cache/sylphx/models`. After that everything runs offline, and no API is called.
  - `repomap model` fetches it ahead of time. `REPOMAP_EMBED=0` keeps search keyword-only, and so does a machine with no network.
  - The MCP server starts at once and picks the model up when the download finishes.
  - The Docker images ship with the model.
- **Better ranking for every query, with or without the model.** The ranking now combines names, keywords and meaning. On top of that:
  - Files whose name or folder matches the question rank higher.
  - Results spread across files.
  - A file with several matching chunks is lifted.
  - Tests, examples, docs and compatibility shims rank lower.
  - On [semble's public benchmark](https://sylphxai.github.io/repomap/benchmarks) (63 repositories, 1,251 questions), NDCG@10 rose from 0.685 to 0.851. semble scored 0.851 on the same runner. Without the model the score is 0.810. On our 60 questions over Django, Kubernetes, VS Code and rust-analyzer it rose from 0.463 to 0.794, against 0.799 for semble.
  - A cold index takes 16–19% longer with embeddings, well under the 1.5× budget.
- **One-click install.** Each GitHub release now carries MCP Bundles (`.mcpb`) for Claude Desktop and other MCPB hosts. There is one bundle for all platforms and one per platform. The bundle asks for a project folder.
- **Kotlin:** a class body closed on the same line (`class P { val a = 1 }`) no longer drops a file's symbols. This works around an unmerged tree-sitter-kotlin fix.
- **Fixes:**
  - A library file named `test_*.bash` or `test_*.ts` is no longer treated as a test. Only `test_*.py`, `.rb`, `.c` and similar are.
  - CI shows the queue-only job as `test other OS`, not `test (${{ matrix.os }})`.
- **mcp-kit 0.2** from crates.io (`sylphx-mcp-kit`) instead of a git tag.

## 1.2.2

- **MCP server** now runs on [mcp-kit](https://github.com/SylphxAI/mcp-kit), which uses rmcp, the official Rust MCP SDK, instead of repomap's own JSON-RPC loop. Tools, tool names and answers are unchanged. The server now also handles protocol negotiation across every spec version, cancellation, progress and pagination.
- **Shared parts:** `setup`, the npm launcher and the release workflow now come from mcp-kit, shared with the other Sylphx MCP servers.

## 1.2.1

- **Security (`serve`, `db --serve`):**
  - Query and `file://` URI percent-decoding now works on bytes. Malformed input such as `?q=%aé` could panic a request thread.
  - Requests are served by a fixed pool of 8 workers instead of one thread per request.
  - A non-loopback `--host` now always requires a token. Pass `--token` or `REPOMAP_TOKEN`, or repomap generates one and prints it in the URL. The token is accepted as `?token=`, a Bearer header, or an HttpOnly cookie, and compared in constant time.
- **Release:** npm packages publish with trusted publishing (OIDC, with provenance) instead of a long-lived token.
- **Copy:**
  - The README tool table is generated from the tool registry (`repomap tools`). There are six tools, including `db`.
  - One one-liner (`brand.json`) feeds the README, docs, npm, the MCP Registry and the GitHub description, and CI checks that they match.
  - Migration notes now live in one place.
- **Score:** `repomap score --badge-style static` writes a self-hosted `.github/agent-ready.svg` for people who prefer not to use the hosted badge. The Action takes `badge-style: static`.

## 1.2.0

- **Database map.** `repomap db` and the `db` MCP tool map tables, columns, keys and indexes, and link every table to the code that queries it (raw SQL, Prisma, Drizzle, SQLAlchemy, Diesel).
  - Sources: SQL migrations (in order, rollbacks skipped), Prisma, Drizzle, SQLAlchemy and Flask-SQLAlchemy, and Diesel.
  - Live Postgres, MySQL and SQLite are strictly read-only:
    - Postgres uses a read-only transaction and verifies it before reading anything.
    - MySQL uses `START TRANSACTION READ ONLY`.
    - SQLite opens the file read-only with `query_only`.
    - Only catalog metadata is read.
    - The connection string comes from an argument or an env var and is never stored or printed.
  - `--serve` / `--out` render tables and foreign keys in the graph UI, with a "Queried from" panel.
- **Agent-readiness score.** `repomap score` rates a repository from 0 to 100 on eight checks and lists concrete fixes.
  - The checks: agent instructions, build/test commands, tests, CI, module boundaries, file sizes, docs and types.
  - It prints a README badge (`mark.sylphx.com/badge/agent--ready-…`).
  - Flags: `--min` gates CI; `--update-readme` refreshes the badge.
- **GitHub Action.** `uses: SylphxAI/repomap@v1` scores on CI, writes the job summary, and can keep the README badge current.

## 1.1.2

- **Docker image** `ghcr.io/sylphxai/repomap` (linux/amd64, linux/arm64), published on every release. It runs the stdio MCP server; mount the repo at `/workspace`.
- The MCP server skips client roots that do not exist locally (container clients see host paths), falling back to the working directory.
- The release workflow marks the old MCP Registry names `io.github.SylphxAI/spine` and `io.github.SylphxAI/locus` as deprecated, pointing at repomap.

## 1.1.1

- `setup --claude-hooks` run through `npx` no longer writes `repomap hook` (a temporary npx shim) into Claude Code settings. It writes `npx -y @sylphx/repomap hook` unless repomap is installed globally.

## 1.1.0

- **Better modules.** Tests, examples, docs and benchmarks are detected by path (including `jvmTest`, `runtime-tests`, `watchOS Example`) and grouped by role. They no longer join or name core modules. Modules are named after their dominant directory with generic segments dropped (`tokio/runtime`, `flask/sansio`, `okhttp/okhttp3`). Name clashes are settled by the sub-directory that sets a module apart, or by its central file. Unconnected stragglers go into one `other` group instead of many one-file modules.
- **Kotlin and Swift** now get a symbol graph: definitions, calls, imports (Kotlin) and inheritance.
- **Claude Code hook.** `repomap setup --claude-hooks` installs an opt-in PreToolUse hook that adds repomap context (definition, callers, module) to Grep and Glob. `repomap hook` is the command; `setup --remove` removes it.
- **Graph UI polish:**
  - Selecting a file no longer over-zooms, and it stays clear of the side panel.
  - The canvas sits beside the module legend.
  - Search is centred over the free space.
  - Palette colours are more distinct.
  - Tests, examples and docs are muted and hidden by default in bigger repos.
- **Live demo:** `/demo` on the docs site hosts exported maps of excalidraw, axum and flask, rebuilt by CI.
- **README demo GIF**, recorded from the real UI (`scripts/record-demo.py`).

## 1.0.1

- Old launch commands keep working: `npx @sylphx/locus --root=/repo` (flags with no subcommand) starts the MCP server, and `LOCUS_ROOT`/`CODERAG_ROOT` are honoured.
- `map --limit` now also caps modules and entry points.
- `setup --dry-run` says what *would* change.
- Graph UI: impact view labels only the selected file and the central direct dependents.
- Release: publish every package first, then wait once for the npm registry, so a slow registry does not fail the run.

## 1.0.0

repomap is Spine and Locus, merged and rebuilt.

- New engine: tree-sitter extraction for TypeScript, TSX, JavaScript, Python, Go,
  Rust, Java, C, C++, C#, Ruby and PHP; resolved import, call and inheritance
  graph; PageRank; Louvain modules; BM25 over AST chunks; per-file cache; and
  `.gitignore` support.
- Five MCP tools: `map`, `search`, `context`, `trace` and `impact` (including
  `changed: true` for the current git diff). The old `architecture_*` and
  `codebase_search` names are still accepted until 2.0.
- `repomap serve`: an interactive WebGL graph UI with search, an impact and
  dependency view, and code preview. `repomap export`: a self-contained HTML map.
- `repomap setup`: one command that configures Claude Code, Codex, Cursor,
  VS Code, Claude Desktop, Windsurf and Gemini CLI.
- Native binaries for macOS (arm64, x64), Linux glibc (x64, arm64) and
  Windows x64.
- `@sylphx/spine`, `@sylphx/locus` and `@sylphx/coderag` are now aliases of
  `@sylphx/repomap`.
