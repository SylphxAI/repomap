# repomap: agent notes

Read [PROJECT.md](PROJECT.md) for the layout and the release flow.

- Run the narrowest check first: `cargo test -p repomap-core`, then
  `cargo test --workspace`. Big benchmarks run in CI (`bench.yml`), not locally.
- After changing `ui/src/*`, run `bun run build:ui` and commit
  `crates/repomap/assets/` (CI checks that it is fresh).
- To add a language, add a grammar crate and a query in
  `crates/repomap-core/src/lang.rs`, following the capture conventions at the top
  of that file. Then add a case to the `parse.rs` tests.
- Keep the MCP surface at five tools. Legacy names are routed in
  `crates/repomap/src/tools.rs::canonical` until 2.0.
- One version everywhere: use `bun scripts/set-version.ts` (CI runs
  `scripts/check-version.ts`).
- Never commit secrets, tokens or `.env` files.
