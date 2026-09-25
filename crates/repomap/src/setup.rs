//! `repomap setup`: register repomap with MCP clients (mcp-kit does the
//! editing) and, with `--claude-hooks`, add the Grep/Glob hook.

use anyhow::Result;
use mcp_kit::setup::{self, Change, Hook, Options, Server};
use std::collections::HashMap;

pub fn run(flags: &HashMap<String, String>) -> Result<()> {
    let opts = Options {
        dry_run: flags.contains_key("dry-run"),
        remove: flags.contains_key("remove"),
        clients: flags.get("client").map(|c| c.split(',').map(|s| s.trim().to_string()).collect()),
    };
    println!("repomap setup{}", if opts.dry_run { " (dry run)" } else { "" });
    let server = Server { name: "repomap".into(), package: "@sylphx/repomap".into(), args: vec!["mcp".into()] };
    let mut touched = setup::run(&server, &opts)?;

    // Claude Code hook: opt in with --claude-hooks; --remove always cleans it up.
    let want_hooks = flags.contains_key("claude-hooks") || flags.contains_key("hooks");
    let settings = setup::claude_settings();
    if want_hooks || (opts.remove && settings.exists()) {
        // A native `repomap` on PATH starts in milliseconds; otherwise go through npx.
        let command = if setup::installed("repomap") { "repomap hook" } else { "npx -y @sylphx/repomap hook" };
        let hook = Hook { event: "PreToolUse".into(), matcher: "Grep|Glob".into(), command: command.into(), marker: "repomap hook".into(), timeout_secs: 10 };
        match setup::claude_hook(&settings, &hook, &opts) {
            Ok(Change::Unchanged) if want_hooks => println!("  = {:<17} Grep/Glob hook already installed ({})", "Claude Code hook", settings.display()),
            Ok(Change::Unchanged) => {}
            Ok(Change::Wrote(what)) => {
                touched += 1;
                println!("  + {:<17} {} {} (PreToolUse Grep|Glob -> `{command}`)", "Claude Code hook", setup::describe(what, opts.dry_run), settings.display());
                if !opts.remove && command.starts_with("npx") {
                    println!("    Tip: `npm i -g @sylphx/repomap` makes the hook start faster (it then runs `repomap hook`).");
                }
            }
            Err(e) => println!("  ! {:<17} {e}", "Claude Code hook"),
        }
    } else if !opts.dry_run && !opts.remove {
        println!("  Tip: add --claude-hooks to enrich Claude Code's Grep/Glob with repomap context.");
    }
    if touched > 0 && !opts.dry_run {
        println!("\nDone. Restart your editor or agent, then ask it: \"Use repomap to map this repo.\"");
    }
    Ok(())
}
