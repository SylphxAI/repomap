# Claude Code hook

Agents reach for `Grep` and `Glob` out of habit. With the hook installed, every such search in Claude Code also gets a short note from repomap: where the symbol is defined, who calls it (with `file:line`), and which module it belongs to. Glob searches get the matching files and how central they are. The search itself still runs; the hook only adds context.

```bash
npx -y @sylphx/repomap setup --claude-hooks     # install (idempotent)
npx -y @sylphx/repomap setup --remove           # remove repomap, hook included
```

This adds one entry to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      { "matcher": "Grep|Glob", "hooks": [{ "type": "command", "command": "repomap hook", "timeout": 10 }] }
    ]
  }
}
```

When `repomap` is not on your `PATH`, the command is `npx -y @sylphx/repomap hook`. For a faster hook, run `npm i -g @sylphx/repomap` and then `setup --claude-hooks` again.

## What Claude sees

For `Grep` with the pattern `function compose\(` in hono:

```text
repomap (code map) for this search:
- function `compose` defined at src/compose.ts:15 (module router); 3 callers: Hono.route (src/hono-base.ts:228), Hono.#dispatch (src/hono-base.ts:452), every (src/middleware/combine/index.ts:102)
Next: repomap `context <symbol>` for code + callers, `impact <symbol>` before editing.
```

## Behaviour

- It reads the index from the per-file cache, so it answers in tens of milliseconds on most repos. If the index is not ready within 4 s (`REPOMAP_HOOK_TIMEOUT_MS`), it stays silent.
- It never blocks or changes the tool call, and on any error it prints nothing.
- It only works inside a git repository, and never in your home directory.
- Output is capped at about 1,800 characters.
