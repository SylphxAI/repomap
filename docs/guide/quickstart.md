# Quickstart

## 1. Connect your agent

```bash
npx -y @sylphx/repomap setup
```

`setup` finds the MCP clients you have (Claude Code, Codex, Cursor, VS Code, Claude Desktop, Windsurf and Gemini CLI), adds repomap to each one, and prints what it changed. It is safe to run again. Use `--dry-run` to preview, `--client cursor,codex` to pick clients, and `--remove` to undo.

Restart the client, then ask:

> Use repomap to map this repo, then show me what would break if I changed `SessionStore.refresh`.

## 2. See the map

```bash
cd your-repo
npx -y @sylphx/repomap serve
```

Your browser opens the [graph UI](/guide/ui). To publish a snapshot, run `npx -y @sylphx/repomap export`. It writes `repomap.html`, which you can open anywhere.

## 3. Use it from the terminal

```bash
npx -y @sylphx/repomap map                       # overview
npx -y @sylphx/repomap search "retry backoff"    # hybrid search
npx -y @sylphx/repomap context Client.send       # 360° view
npx -y @sylphx/repomap trace main handleRequest  # call path
npx -y @sylphx/repomap impact --changed          # blast radius of your diff
```

For a global install, run `npm i -g @sylphx/repomap`, then use `repomap …`.

## Supported platforms

macOS (Apple silicon, Intel), Linux glibc (x64, arm64) and Windows x64. Node 18+ is only used to launch the native binary. You can also download a binary from [GitHub releases](https://github.com/SylphxAI/repomap/releases) or build one with `cargo install --git https://github.com/SylphxAI/repomap repomap`.
