# Editors and agents

`npx -y @sylphx/repomap setup` configures every client below that it detects. Each client runs the same command:

```json
{ "command": "npx", "args": ["-y", "@sylphx/repomap", "mcp"] }
```

| Client | Config written |
|---|---|
| Claude Code | `claude mcp add --scope user repomap -- npx -y @sylphx/repomap mcp` (or `~/.claude.json`) |
| Codex | `~/.codex/config.toml` → `[mcp_servers.repomap]` |
| Cursor | `~/.cursor/mcp.json` → `mcpServers.repomap` |
| VS Code (+ Insiders) | `<user dir>/mcp.json` → `servers.repomap` |
| Claude Desktop | `claude_desktop_config.json` → `mcpServers.repomap` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` |
| Gemini CLI | `~/.gemini/settings.json` |

On Windows the command is `cmd /c npx …`.

## Claude Desktop, one click

Each [release](https://github.com/SylphxAI/repomap/releases/latest) carries MCP Bundles:

- `repomap-<version>.mcpb` works on every platform.
- `repomap-<version>-<platform>.mcpb` is one platform's binary, about a fifth of the size.

Open one in Claude Desktop, or another MCPB host, and pick the project folder repomap maps by default. The agent can pass another folder in each call.

## Docker

```bash
docker run -i --rm -v "$PWD:/workspace:ro" ghcr.io/sylphxai/repomap
```

The image runs the stdio MCP server against `/workspace` and is published for linux/amd64 and linux/arm64 on every release.

## Claude Code hook (optional)

`setup --claude-hooks` also installs a PreToolUse hook. Whenever Claude Code runs `Grep` or `Glob`, the hook adds where the symbol is defined and who calls it. See [Claude Code hook](/guide/claude-code-hook).

## Which repository is indexed

In order of precedence:

1. the `root` argument of a tool call
2. `REPOMAP_ROOT`
3. `repomap mcp --root <dir>`
4. the client's workspace root (MCP roots)
5. the server's working directory (Claude Code, Codex and Cursor start it in your project)

Claude Desktop has no project directory, so ask it to pass `root`, for example: "map /Users/me/code/app".

## Options

- `REPOMAP_CACHE_DIR`: where per-file parse caches live (default: your OS cache dir, `…/repomap/<repo>-<hash>`)
- `REPOMAP_LEGACY_TOOLS=1`: also list the old `architecture_*` and `codebase_search` names in `tools/list`
- `.repomapignore`: extra ignore patterns in `.gitignore` syntax

## The search model

The first index downloads a 33 MB code embedding model from Hugging Face, prints one line about it, and checks it against a pinned SHA-256. It is stored in `~/.cache/sylphx/models` and used offline from then on.

- `repomap model` downloads it now, for example before going offline or in a Dockerfile. The Docker images already contain it.
- `SYLPHX_MODEL_DIR` moves the model folder, and `SYLPHX_MODEL_URL` points at a mirror that serves the same files.
- `REPOMAP_EMBED=0` turns embeddings off. Search is then keyword-only: symbol names and BM25.
- With no network, repomap stays keyword-only and tries the download again after an hour.
