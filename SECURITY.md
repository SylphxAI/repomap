# Security

## Reporting

Report vulnerabilities privately via GitHub Security Advisories on
[SylphxAI/repomap](https://github.com/SylphxAI/repomap/security/advisories/new).
Do not open public issues for sensitive reports.

## Boundary

- repomap makes no network calls while indexing or answering queries. The only
  subprocess it runs is `git` (commit, remote, diff), plus the `claude` CLI in
  `repomap setup`.
- The MCP server speaks stdio only.
- `repomap serve` binds to `127.0.0.1` by default and rejects requests whose
  `Host` header is not a loopback address (DNS-rebinding guard). The API is
  read-only and only reads files that are in the index for the served root.
- `repomap export` embeds file paths, symbol names and line numbers, but no
  source code. Review the file before you publish it for a private repository.
- The parse cache lives in your OS cache directory (`REPOMAP_CACHE_DIR` overrides it) and
  holds symbol names and token counts from your code.
