//! MCP server over stdio (newline-delimited JSON-RPC 2.0).

use crate::tools;
use crate::workspace::Workspace;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

const PROTOCOLS: [&str; 4] = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "repomap is a map of this codebase. Start with `map` to see modules, central files and key symbols. Use `search` to find code by words or identifiers, `context` for a 360° view of a symbol or file (code, callers, callees, tests), `trace` for call paths, and `impact` before editing to see what could break (or `impact` with changed=true to review the current diff). `db` maps the database schema and where the code queries each table. Every answer cites file:line.";

struct Roots {
    list: Mutex<Option<Vec<PathBuf>>>,
    ready: Condvar,
}

pub fn serve(default_root: Option<PathBuf>) -> anyhow::Result<()> {
    let ws = Arc::new(Workspace::default());
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let roots = Arc::new(Roots { list: Mutex::new(None), ready: Condvar::new() });
    let mut client_roots = false;
    let legacy = std::env::var("REPOMAP_LEGACY_TOOLS").map_or(false, |v| v == "1" || v == "true");

    let send = {
        let out = out.clone();
        move |v: Value| {
            let mut o = out.lock().unwrap();
            let _ = writeln!(o, "{v}");
            let _ = o.flush();
        }
    };

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                send(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}}));
                continue;
            }
        };
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        // Responses to our own requests (roots/list).
        if method.is_empty() {
            if id.as_ref().and_then(|v| v.as_str()).map_or(false, |s| s.starts_with("roots")) {
                let list: Vec<PathBuf> = msg
                    .pointer("/result/roots")
                    .and_then(|r| r.as_array())
                    .map(|a| a.iter().filter_map(|r| r.get("uri").and_then(|u| u.as_str())).filter_map(file_uri_to_path).collect())
                    .unwrap_or_default();
                *roots.list.lock().unwrap() = Some(list);
                roots.ready.notify_all();
            }
            continue;
        }

        match method {
            "initialize" => {
                let asked = msg.pointer("/params/protocolVersion").and_then(|v| v.as_str()).unwrap_or(PROTOCOLS[0]);
                let version = if PROTOCOLS.contains(&asked) { asked } else { PROTOCOLS[0] };
                client_roots = msg.pointer("/params/capabilities/roots").is_some();
                if !client_roots {
                    *roots.list.lock().unwrap() = Some(Vec::new());
                }
                send(json!({"jsonrpc": "2.0", "id": id, "result": {
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "repomap", "title": "repomap", "version": crate::VERSION, "websiteUrl": "https://sylphxai.github.io/repomap/"},
                    "instructions": INSTRUCTIONS
                }}));
            }
            "notifications/initialized" | "notifications/roots/list_changed" => {
                if client_roots {
                    if method == "notifications/roots/list_changed" {
                        *roots.list.lock().unwrap() = None;
                    }
                    send(json!({"jsonrpc": "2.0", "id": format!("roots-{}", rand_id()), "method": "roots/list"}));
                }
                // Warm the index in the background so the first call is fast.
                let ws = ws.clone();
                let roots = roots.clone();
                let default_root = default_root.clone();
                std::thread::spawn(move || {
                    if let Ok(root) = pick_root(None, default_root.as_ref(), &roots) {
                        let _ = ws.get(&root);
                    }
                });
            }
            "ping" => send(json!({"jsonrpc": "2.0", "id": id, "result": {}})),
            "tools/list" => send(json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tools::definitions(legacy)}})),
            "tools/call" => {
                let ws = ws.clone();
                let roots = roots.clone();
                let default_root = default_root.clone();
                let send = send.clone();
                std::thread::spawn(move || {
                    let name = msg.pointer("/params/name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let args = msg.pointer("/params/arguments").cloned().unwrap_or(json!({}));
                    let explicit = args.get("root").or_else(|| args.get("repo_root")).and_then(|v| v.as_str()).map(PathBuf::from);
                    let result = pick_root(explicit, default_root.as_ref(), &roots)
                        .and_then(|root| tools::call(&ws, &name, &args, &root));
                    let want_json = args.get("format").and_then(|v| v.as_str()) == Some("json");
                    let body = match result {
                        Ok(o) => {
                            let text = if want_json { serde_json::to_string_pretty(&o.json).unwrap_or_default() } else { o.text };
                            json!({"content": [{"type": "text", "text": text}], "isError": false})
                        }
                        Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
                    };
                    send(json!({"jsonrpc": "2.0", "id": id, "result": body}));
                });
            }
            "resources/list" => send(json!({"jsonrpc": "2.0", "id": id, "result": {"resources": []}})),
            "prompts/list" => send(json!({"jsonrpc": "2.0", "id": id, "result": {"prompts": []}})),
            m if m.starts_with("notifications/") => {}
            _ => {
                if id.is_some() {
                    send(json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("method not found: {method}")}}));
                }
            }
        }
    }
    Ok(())
}

fn rand_id() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
}

fn pick_root(explicit: Option<PathBuf>, default_root: Option<&PathBuf>, roots: &Roots) -> Result<PathBuf, String> {
    if let Some(r) = explicit {
        return check(r);
    }
    for var in ["REPOMAP_ROOT", "LOCUS_ROOT", "CODERAG_ROOT"] {
        if let Ok(r) = std::env::var(var) {
            if !r.is_empty() {
                return check(PathBuf::from(r));
            }
        }
    }
    if let Some(r) = default_root {
        return check(r.clone());
    }
    // Wait briefly for the client's roots answer.
    let guard = roots.list.lock().unwrap();
    let (guard, _) = roots.ready.wait_timeout_while(guard, Duration::from_secs(3), |l| l.is_none()).unwrap();
    // Client roots are host paths; inside a container they may not exist.
    if let Some(first) = guard.as_ref().and_then(|l| l.iter().find(|p| p.is_dir())) {
        return check(first.clone());
    }
    drop(guard);
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let home = dirs::home_dir();
    if cwd.parent().is_none() || Some(&cwd) == home.as_ref() {
        return Err("No repository selected: pass `root` (absolute path to the repository), or start the server from the project directory.".into());
    }
    check(cwd)
}

fn check(p: PathBuf) -> Result<PathBuf, String> {
    if p.is_dir() {
        Ok(p)
    } else {
        Err(format!("root `{}` is not a directory", p.display()))
    }
}

pub fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let mut bytes = Vec::with_capacity(rest.len());
    let b = rest.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            // Byte-wise: `&rest[i+1..i+3]` could split a UTF-8 character and panic.
            let h = |c: u8| (c as char).to_digit(16);
            if let (Some(x), Some(y)) = (h(b[i + 1]), h(b[i + 2])) {
                bytes.push((x * 16 + y) as u8);
                i += 3;
                continue;
            }
        }
        bytes.push(b[i]);
        i += 1;
    }
    let s = String::from_utf8(bytes).ok()?;
    // file:///C:/x on Windows
    let s = if s.len() > 3 && s.as_bytes()[0] == b'/' && s.as_bytes()[2] == b':' { s[1..].to_string() } else { s };
    Some(PathBuf::from(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_never_panic() {
        assert_eq!(file_uri_to_path("file:///a%20b/c").unwrap(), PathBuf::from("/a b/c"));
        for u in ["file:///%aé", "file:///%", "file:///%é%", "file:///中%2", "file:///%zz"] {
            let _ = file_uri_to_path(u);
        }
    }
}
