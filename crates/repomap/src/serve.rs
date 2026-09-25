//! `repomap serve`: local web UI over a tiny HTTP server, and the
//! self-contained static HTML export.

use crate::tools;
use crate::workspace::Workspace;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;
use tiny_http::{Header, Method, Response, Server};

const INDEX: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");
const APP_CSS: &str = include_str!("../assets/app.css");

fn page(title: &str, css: &str, js: &str, data: &str) -> String {
    INDEX
        .replace("__TITLE__", title)
        .replace("__VERSION__", crate::VERSION)
        .replace("__CSS__", css)
        .replace("__DATA__", data)
        .replace("__JS__", js)
}

/// A single HTML file with the UI and the graph inlined.
pub fn static_html(data: &Value) -> String {
    let json = serde_json::to_string(data).unwrap_or_default().replace("</", "<\\/");
    let title = format!("{} · repomap", data["repo"].as_str().unwrap_or("repo"));
    page(
        &html_escape(&title),
        &format!("<style>{APP_CSS}</style>"),
        &format!("<script>{}</script>", APP_JS.replace("</script", "<\\/script")),
        &format!("<script>window.__REPOMAP__={json};</script>"),
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn run(root: &Path, host: &str, port: u16, open: bool) -> Result<()> {
    let root = root.canonicalize().with_context(|| format!("cannot open {}", root.display()))?;
    let ws = Arc::new(Workspace::default());
    let t = std::time::Instant::now();
    let idx = ws.get(&root)?;
    eprintln!(
        "repomap: indexed {} files, {} symbols in {} ms",
        idx.code_files().count(),
        idx.symbols.len(),
        t.elapsed().as_millis()
    );
    let mut server = None;
    let mut bound = port;
    for p in port..port.saturating_add(20) {
        if let Ok(s) = Server::http((host, p)) {
            server = Some(s);
            bound = p;
            break;
        }
    }
    let server = server.ok_or_else(|| anyhow::anyhow!("no free port from {port}"))?;
    let url = format!("http://{}:{bound}/", if host == "0.0.0.0" { "localhost" } else { host });
    eprintln!("repomap: serving {} at {url}  (Ctrl+C to stop)", root.display());
    if open {
        open_browser(&url);
    }
    let title = format!("{} · repomap", root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let shell = page(
        &html_escape(&title),
        "<link rel=\"stylesheet\" href=\"app.css\">",
        "<script src=\"app.js\"></script>",
        "",
    );
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1");
    let allowed: Vec<String> = ["127.0.0.1", "localhost", "[::1]"].iter().map(|h| format!("{h}:{bound}")).collect();
    for req in server.incoming_requests() {
        // DNS-rebinding guard: a loopback server only answers loopback Host headers.
        if loopback {
            let host_ok = req
                .headers()
                .iter()
                .find(|h| h.field.equiv("Host"))
                .map_or(false, |h| allowed.iter().any(|a| a == h.value.as_str()));
            if !host_ok {
                let _ = req.respond(text(403, "forbidden host", "text/plain"));
                continue;
            }
        }
        let ws = ws.clone();
        let root = root.clone();
        let shell = shell.clone();
        std::thread::spawn(move || {
            let url = req.url().to_string();
            let (path, query) = url.split_once('?').unwrap_or((&url, ""));
            let q = parse_query(query);
            let resp = if *req.method() != Method::Get {
                text(405, "method not allowed", "text/plain")
            } else {
                route(&ws, &root, path, &q, &shell)
            };
            let _ = req.respond(resp);
        });
    }
    Ok(())
}

type Resp = Response<std::io::Cursor<Vec<u8>>>;

fn text(code: u16, body: &str, ctype: &str) -> Resp {
    Response::from_data(body.as_bytes().to_vec())
        .with_status_code(code)
        .with_header(Header::from_bytes("Content-Type", ctype).unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
}

fn json_resp(v: &Value) -> Resp {
    text(200, &serde_json::to_string(v).unwrap_or_default(), "application/json; charset=utf-8")
}

fn route(ws: &Workspace, root: &Path, path: &str, q: &std::collections::HashMap<String, String>, shell: &str) -> Resp {
    match path {
        "/" | "/index.html" => text(200, shell, "text/html; charset=utf-8"),
        "/app.js" => text(200, APP_JS, "text/javascript; charset=utf-8"),
        "/app.css" => text(200, APP_CSS, "text/css; charset=utf-8"),
        "/api/graph" => match ws.get(root) {
            Ok(idx) => {
                let mut v = idx.graph_json(&Default::default(), crate::VERSION);
                v["root"] = json!(idx.root.to_string_lossy().replace('\\', "/"));
                json_resp(&v)
            }
            Err(e) => text(500, &e.to_string(), "text/plain"),
        },
        "/api/search" | "/api/context" | "/api/impact" | "/api/trace" | "/api/map" => {
            let tool = &path[5..];
            let mut args = serde_json::Map::new();
            for (k, v) in q {
                let val = match v.parse::<u64>() {
                    Ok(n) if k != "q" && k != "query" && k != "target" => json!(n),
                    _ => json!(v),
                };
                args.insert(if k == "q" { "query".into() } else { k.clone() }, val);
            }
            match tools::call(ws, tool, &Value::Object(args), root) {
                Ok(o) => json_resp(&o.json),
                Err(e) => text(400, &json!({"error": e}).to_string(), "application/json"),
            }
        }
        _ => text(404, "not found", "text/plain"),
    }
}

fn parse_query(q: &str) -> std::collections::HashMap<String, String> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            Some((decode(k), decode(v)))
        })
        .collect()
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn open_browser(url: &str) {
    let r = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else if cfg!(windows) {
        std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()
    };
    if r.is_err() {
        eprintln!("repomap: open {url} in your browser");
    }
}

/// Serve one prebuilt page (used by `repomap db --serve`).
pub fn serve_static(html: &str, host: &str, port: u16, open: bool) -> Result<()> {
    let mut server = None;
    let mut bound = port;
    for p in port..port.saturating_add(20) {
        if let Ok(s) = Server::http((host, p)) {
            server = Some(s);
            bound = p;
            break;
        }
    }
    let server = server.ok_or_else(|| anyhow::anyhow!("no free port from {port}"))?;
    let url = format!("http://{}:{bound}/", if host == "0.0.0.0" { "localhost" } else { host });
    eprintln!("repomap: serving the database map at {url}  (Ctrl+C to stop)");
    if open {
        open_browser(&url);
    }
    for req in server.incoming_requests() {
        let resp = if req.url() == "/" || req.url().starts_with("/#") || req.url() == "/index.html" { text(200, html, "text/html; charset=utf-8") } else { text(404, "not found", "text/plain") };
        let _ = req.respond(resp);
    }
    Ok(())
}
