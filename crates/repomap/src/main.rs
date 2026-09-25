mod dblive;
mod hook;
mod mcp;
mod serve;
mod setup;
mod tools;
mod workspace;

use anyhow::{bail, Result};
use repomap_core::{BuildOptions, Index};
use serde_json::{json, Value};
use std::io::IsTerminal;
use std::path::PathBuf;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const HELP: &str = "repomap — a map of your codebase for you and your AI agent

Usage: repomap <command> [options]

Commands:
  setup                 Add repomap to Claude Code, Codex, Cursor, VS Code, Claude Desktop, Windsurf
                        (--claude-hooks also enriches Claude Code's Grep/Glob; --remove undoes)
  serve [dir]           Open the interactive graph UI in your browser (alias: ui)
                        (--host 0.0.0.0 requires a token: --token/REPOMAP_TOKEN, or one is generated)
  export [dir]          Write a self-contained HTML map (--out repomap.html) or --json
  map [dir]             Modules, central files, key symbols (--focus <dir> to zoom in)
  search <query>        Hybrid symbol + BM25 code search
  context <target>      Code, callers, callees, tests for a symbol or file
  trace <from> [to]     Call path between two symbols, or a call tree (--callers)
  impact [target...]    Blast radius of a change (--changed for the current git diff)
  db [table]            Database map: tables, keys, indexes and the code that queries them
                        (repo schema files, or live read-only via --url-env DATABASE_URL;
                        --serve / --out map.html for the graph)
  score [dir]           Agent-readiness score (0-100) with fixes and a README badge
                        (--update-readme README.md, --min 70 to fail CI below a score,
                        --badge-style static to write a self-hosted .github/agent-ready.svg)
  index [dir]           Build the index and print timings (--no-cache, --json)
  mcp                   Run the MCP server on stdio (default when stdin is not a terminal)
  version               Print the version

Common options:
  -C, --root <dir>      Repository root (default: current directory)
  --json                Machine-readable output

Targets: path/to/file.ts, file.ts:42, Class.method, Class::method, or a name.
Docs: https://sylphxai.github.io/repomap/";

struct Args {
    positional: Vec<String>,
    flags: std::collections::HashMap<String, String>,
}

impl Args {
    fn parse(raw: Vec<String>) -> Args {
        let mut positional = Vec::new();
        let mut flags = std::collections::HashMap::new();
        let takes_value = ["badge-style", "badge-file", "token", "update-readme", "min", "url", "url-env", "table", "root", "C", "focus", "limit", "path", "kind", "depth", "base", "port", "host", "out", "json-out", "client", "code-lines", "command"];
        let mut it = raw.into_iter().peekable();
        while let Some(a) = it.next() {
            if let Some(name) = a.strip_prefix("--").or_else(|| a.strip_prefix('-').filter(|n| n.len() == 1)) {
                if let Some((k, v)) = name.split_once('=') {
                    flags.insert(k.to_string(), v.to_string());
                } else if takes_value.contains(&name) {
                    let v = it.next().unwrap_or_default();
                    flags.insert(name.to_string(), v);
                } else {
                    flags.insert(name.to_string(), "true".into());
                }
            } else {
                positional.push(a);
            }
        }
        Args { positional, flags }
    }
    fn flag(&self, k: &str) -> Option<&str> {
        self.flags.get(k).map(|s| s.as_str())
    }
    fn on(&self, k: &str) -> bool {
        self.flags.get(k).map_or(false, |v| v != "false")
    }
    fn num(&self, k: &str) -> Option<usize> {
        self.flag(k).and_then(|v| v.parse().ok())
    }
    fn root(&self) -> PathBuf {
        self.flag("root")
            .or_else(|| self.flag("C"))
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }
    /// Root from `--root` or the first positional directory argument.
    fn root_or_pos(&self, pos: usize) -> PathBuf {
        if self.flag("root").is_some() || self.flag("C").is_some() {
            return self.root();
        }
        self.positional.get(pos).map(PathBuf::from).unwrap_or_else(|| self.root())
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("repomap: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut raw: Vec<String> = std::env::args().skip(1).collect();
    if raw.is_empty() {
        if std::io::stdin().is_terminal() {
            println!("{HELP}");
            return Ok(());
        }
        return mcp::serve(None);
    }
    // Legacy launches (`npx @sylphx/locus --root=/repo`) start with a flag:
    // treat them as the MCP server.
    if raw[0].starts_with('-') && !matches!(raw[0].as_str(), "-h" | "--help" | "-V" | "--version") {
        raw.insert(0, "mcp".into());
    }
    let cmd = raw.remove(0);
    let args = Args::parse(raw);
    match cmd.as_str() {
        "mcp" => mcp::serve(args.flag("root").or_else(|| args.flag("C")).map(PathBuf::from)),
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("repomap {VERSION}");
            Ok(())
        }
        "setup" => setup::run(&args.flags),
        "hook" => {
            hook::run();
            Ok(())
        }
        "serve" | "ui" => serve::run(
            &args.root_or_pos(0),
            args.flag("host").unwrap_or("127.0.0.1"),
            args.num("port").unwrap_or(7878) as u16,
            !args.on("no-open"),
            args.flag("token").map(String::from).or_else(|| std::env::var("REPOMAP_TOKEN").ok()),
        ),
        "export" => export(&args),
        "index" => index_cmd(&args),
        "map" | "search" | "context" | "trace" | "impact" => query_cmd(&cmd, &args),
        "db" => db_cmd(&args),
        "tools" => {
            if args.on("json") {
                println!("{}", serde_json::to_string_pretty(&tools::definitions(false))?);
            } else {
                print!("{}", tools::markdown());
            }
            Ok(())
        }
        "score" => score_cmd(&args),
        other => bail!("unknown command `{other}`. Run `repomap help`."),
    }
}

fn index_cmd(args: &Args) -> Result<()> {
    let root = args.root_or_pos(0);
    let idx = Index::build(&root, &BuildOptions { use_cache: !args.on("no-cache") })?;
    let s = &idx.stats;
    let v = json!({
        "root": idx.root.display().to_string(),
        "files": idx.files.len(),
        "code_files": idx.code_files().count(),
        "symbols": idx.symbols.len(),
        "call_edges": idx.sym_edges.len(),
        "file_edges": idx.file_edges.len(),
        "chunks": idx.bm25.chunks.len(),
        "terms": idx.bm25.terms(),
        "communities": idx.communities.len(),
        "parsed": s.files_parsed,
        "cached": s.files_cached,
        "walk_ms": s.walk_ms,
        "parse_ms": s.parse_ms,
        "graph_ms": s.graph_ms,
        "total_ms": s.total_ms,
    });
    if args.on("json") {
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        println!(
            "Indexed {} files ({} code) in {} ms: {} symbols, {} call edges, {} file edges, {} modules, {} chunks. Parsed {}, cached {}.",
            v["files"], v["code_files"], s.total_ms, v["symbols"], v["call_edges"], v["file_edges"], v["communities"], v["chunks"], s.files_parsed, s.files_cached
        );
    }
    Ok(())
}

fn query_cmd(cmd: &str, args: &Args) -> Result<()> {
    let root = args.root();
    let mut a = serde_json::Map::new();
    let p = &args.positional;
    match cmd {
        "map" => {
            if let Some(f) = args.flag("focus") {
                a.insert("focus".into(), json!(f));
            }
            // `repomap map some/dir` zooms into that directory when it is inside the root.
            if let Some(first) = p.first() {
                if args.flag("focus").is_none() {
                    a.insert("focus".into(), json!(first));
                }
            }
        }
        "search" => {
            if p.is_empty() {
                bail!("usage: repomap search <query>");
            }
            a.insert("query".into(), json!(p.join(" ")));
            if let Some(v) = args.flag("path") {
                a.insert("path".into(), json!(v));
            }
            if let Some(v) = args.flag("kind") {
                a.insert("kind".into(), json!(v));
            }
        }
        "context" => {
            let t = p.first().ok_or_else(|| anyhow::anyhow!("usage: repomap context <target>"))?;
            a.insert("target".into(), json!(t));
            if let Some(v) = args.num("code-lines") {
                a.insert("code_lines".into(), json!(v));
            }
        }
        "trace" => {
            let f = p.first().ok_or_else(|| anyhow::anyhow!("usage: repomap trace <from> [to]"))?;
            a.insert("from".into(), json!(f));
            if let Some(t) = p.get(1) {
                a.insert("to".into(), json!(t));
            }
            if args.on("callers") {
                a.insert("direction".into(), json!("callers"));
            }
        }
        "impact" => {
            if !p.is_empty() {
                a.insert("target".into(), json!(p));
            }
            if args.on("changed") || p.is_empty() {
                a.insert("changed".into(), json!(true));
            }
            if let Some(v) = args.flag("base") {
                a.insert("base".into(), json!(v));
            }
        }
        _ => {}
    }
    if let Some(v) = args.num("limit") {
        a.insert("limit".into(), json!(v));
    }
    if let Some(v) = args.num("depth") {
        a.insert("depth".into(), json!(v));
    }
    let ws = workspace::Workspace::default();
    let out = tools::call(&ws, cmd, &Value::Object(a), &root).map_err(|e| anyhow::anyhow!(e))?;
    if args.on("json") {
        println!("{}", serde_json::to_string_pretty(&out.json)?);
    } else {
        print!("{}", out.text);
    }
    Ok(())
}

fn export(args: &Args) -> Result<()> {
    let root = args.root_or_pos(0);
    let idx = Index::build(&root, &BuildOptions::default())?;
    let data = idx.graph_json(&Default::default(), VERSION);
    if let Some(path) = args.flag("json-out").or_else(|| if args.on("json") { Some("repomap.json") } else { None }) {
        std::fs::write(path, serde_json::to_vec(&data)?)?;
        eprintln!("Wrote {path}");
        return Ok(());
    }
    let out = args.flag("out").or_else(|| args.flag("html").filter(|v| *v != "true")).unwrap_or("repomap.html");
    std::fs::write(out, serve::static_html(&data))?;
    eprintln!(
        "Wrote {out} ({} files, {} symbols). Open it in a browser or publish it anywhere.",
        data["stats"]["code_files"], data["stats"]["symbols"]
    );
    Ok(())
}

fn db_cmd(args: &Args) -> Result<()> {
    let root = args.root();
    let idx = Index::build(&root, &BuildOptions::default())?;
    let url = match (args.flag("url"), args.flag("url-env")) {
        (Some(u), _) => Some(u.to_string()),
        (None, Some(var)) => Some(std::env::var(var).map_err(|_| anyhow::anyhow!("environment variable `{var}` is not set"))?),
        _ => None,
    };
    let schema = tools::db_schema(&idx, url.as_deref())?;
    let table = args.flag("table").or_else(|| args.positional.first().map(|s| s.as_str()));
    if args.flag("out").is_some() || args.on("serve") {
        let data = schema.graph_json(
            &idx.root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            VERSION,
            idx.git.remote_web.as_deref(),
            idx.git.commit.as_deref(),
        );
        let html = serve::static_html(&data);
        if let Some(out) = args.flag("out") {
            std::fs::write(out, &html)?;
            eprintln!("Wrote {out} ({} tables)", schema.tables.len());
        }
        if args.on("serve") {
            return serve::serve_static(
                &html,
                args.flag("host").unwrap_or("127.0.0.1"),
                args.num("port").unwrap_or(7879) as u16,
                !args.on("no-open"),
                args.flag("token").map(String::from).or_else(|| std::env::var("REPOMAP_TOKEN").ok()),
            );
        }
        return Ok(());
    }
    if args.on("json") {
        let v = match table.and_then(|t| schema.table(t)) {
            Some(t) => serde_json::to_value(t)?,
            None => serde_json::to_value(&schema)?,
        };
        println!("{}", serde_json::to_string_pretty(&v)?);
    } else {
        print!("{}", schema.text(table));
    }
    Ok(())
}

fn score_cmd(args: &Args) -> Result<()> {
    let root = args.root_or_pos(0);
    let idx = Index::build(&root, &BuildOptions::default())?;
    let score = idx.agent_score();
    if args.on("json") {
        println!("{}", serde_json::to_string_pretty(&score)?);
    } else {
        print!("{}", score.text());
    }
    // --badge-style static: a committed SVG instead of the mark.sylphx.com image.
    let mut markdown = score.badge_markdown.clone();
    if args.flag("badge-style") == Some("static") {
        let file = args.flag("badge-file").unwrap_or(".github/agent-ready.svg");
        let path = idx.root.join(file);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, repomap_core::score::badge_svg(score.score))?;
        markdown = repomap_core::score::badge_markdown(score.score, file);
        eprintln!("Wrote {file}\n{markdown}");
    }
    if let Some(readme) = args.flag("update-readme") {
        let path = idx.root.join(readme);
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        match repomap_core::score::update_badge(&text, &markdown, args.on("insert")) {
            Some(new) if new != text => {
                std::fs::write(&path, new)?;
                eprintln!("Updated the agent-ready badge in {readme}");
            }
            Some(_) => eprintln!("{readme}: badge already up to date"),
            None => eprintln!("{readme}: no agent-ready badge found (add --insert to place one under the title)"),
        }
    }
    if let Some(min) = args.num("min") {
        if (score.score as usize) < min {
            bail!("agent-readiness {} is below the minimum {min}", score.score);
        }
    }
    Ok(())
}
