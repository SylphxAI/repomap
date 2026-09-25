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

/// Worker threads that serve requests; more concurrent connections wait in
/// the listener's queue instead of spawning threads without bound.
const WORKERS: usize = 8;

pub fn is_loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]")
}

/// A random 128-bit hex token. `RandomState` is seeded from the OS RNG.
pub fn new_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Who may talk to the server: loopback clients (Host header checked against
/// DNS rebinding), and anyone presenting the token when one is set.
pub struct Guard {
    loopback: bool,
    allowed_hosts: Vec<String>,
    token: Option<String>,
}

impl Guard {
    /// Non-loopback binds always need a token: the given one, or a fresh one.
    pub fn new(host: &str, port: u16, token: Option<String>) -> Guard {
        let loopback = is_loopback(host);
        let token = token.filter(|t| !t.is_empty()).or_else(|| (!loopback).then(new_token));
        let allowed_hosts = ["127.0.0.1", "localhost", "[::1]"].iter().map(|h| format!("{h}:{port}")).collect();
        Guard { loopback, allowed_hosts, token }
    }

    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Ok(set_cookie) when allowed; Err(response) otherwise.
    fn check(&self, req: &tiny_http::Request, query: &std::collections::HashMap<String, String>) -> Result<bool, Resp> {
        let header = |name: &'static str| req.headers().iter().find(|h| h.field.equiv(name)).map(|h| h.value.as_str().to_string());
        if self.loopback && self.token.is_none() {
            let ok = header("Host").map_or(false, |h| self.allowed_hosts.iter().any(|a| *a == h));
            return if ok { Ok(false) } else { Err(text(403, "forbidden host", "text/plain")) };
        }
        let Some(want) = &self.token else { return Ok(false) };
        let presented_query = query.get("token").cloned();
        let presented = presented_query
            .clone()
            .or_else(|| header("Authorization").and_then(|v| v.strip_prefix("Bearer ").map(|t| t.trim().to_string())))
            .or_else(|| header("Cookie").and_then(|c| c.split(';').find_map(|kv| kv.trim().strip_prefix("repomap_token=").map(String::from))));
        match presented {
            Some(p) if ct_eq(p.as_bytes(), want.as_bytes()) => Ok(presented_query.is_some()),
            _ => Err(text(401, "unauthorized: open the URL printed by `repomap serve` (it carries ?token=…)", "text/plain")),
        }
    }
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn bind(host: &str, port: u16) -> Result<(Server, u16)> {
    for p in port..port.saturating_add(20) {
        if let Ok(s) = Server::http((host, p)) {
            return Ok((s, p));
        }
    }
    anyhow::bail!("no free port from {port}")
}

fn announce(what: &str, host: &str, port: u16, guard: &Guard, open: bool) {
    let shown = if host == "0.0.0.0" || host == "::" { "localhost" } else { host };
    let url = match guard.token() {
        Some(t) => format!("http://{shown}:{port}/?token={t}"),
        None => format!("http://{shown}:{port}/"),
    };
    eprintln!("repomap: serving {what} at {url}  (Ctrl+C to stop)");
    if !guard.loopback {
        eprintln!("repomap: listening on {host} (not loopback): every request needs the token above");
    }
    if open {
        open_browser(&url);
    }
}

/// Run `handle` for each request on a fixed pool of worker threads.
fn serve_pool<F>(server: Server, guard: Guard, handle: F)
where
    F: Fn(&tiny_http::Request, &str, &std::collections::HashMap<String, String>) -> Resp + Send + Sync + 'static,
{
    let server = Arc::new(server);
    let guard = Arc::new(guard);
    let handle = Arc::new(handle);
    let workers: Vec<_> = (0..WORKERS)
        .map(|_| {
            let (server, guard, handle) = (server.clone(), guard.clone(), handle.clone());
            std::thread::spawn(move || {
                while let Ok(req) = server.recv() {
                    let url = req.url().to_string();
                    let (path, query) = url.split_once('?').unwrap_or((&url, ""));
                    let q = parse_query(query);
                    let resp = match guard.check(&req, &q) {
                        Err(r) => r,
                        Ok(set_cookie) => {
                            let r = if *req.method() != Method::Get { text(405, "method not allowed", "text/plain") } else { handle(&req, path, &q) };
                            match (set_cookie, guard.token()) {
                                (true, Some(t)) => r.with_header(Header::from_bytes("Set-Cookie", format!("repomap_token={t}; Path=/; HttpOnly; SameSite=Strict")).unwrap()),
                                _ => r,
                            }
                        }
                    };
                    let _ = req.respond(resp);
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
}

pub fn run(root: &Path, host: &str, port: u16, open: bool, token: Option<String>) -> Result<()> {
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
    let (server, bound) = bind(host, port)?;
    let guard = Guard::new(host, bound, token);
    announce(&root.display().to_string(), host, bound, &guard, open);
    let title = format!("{} · repomap", root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
    let shell = page(
        &html_escape(&title),
        "<link rel=\"stylesheet\" href=\"app.css\">",
        "<script src=\"app.js\"></script>",
        "",
    );
    serve_pool(server, guard, move |_req, path, q| route(&ws, &root, path, q, &shell));
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

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode a query component. Works on bytes, so malformed or
/// non-ASCII input (`%aé`, `%`, `%%zz`) can never split a UTF-8 character.
pub fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match (hex(b[i + 1]), hex(b[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push(h << 4 | l);
                        i += 3;
                        continue;
                    }
                    _ => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
pub fn serve_static(html: &str, host: &str, port: u16, open: bool, token: Option<String>) -> Result<()> {
    let (server, bound) = bind(host, port)?;
    let guard = Guard::new(host, bound, token);
    announce("the database map", host, bound, &guard, open);
    let html = html.to_string();
    serve_pool(server, guard, move |_req, path, _q| {
        if path == "/" || path == "/index.html" { text(200, &html, "text/html; charset=utf-8") } else { text(404, "not found", "text/plain") }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_never_panics() {
        assert_eq!(decode("a+b%20c"), "a b c");
        assert_eq!(decode("%e4%b8%ad"), "中");
        assert_eq!(decode("%aé"), "%aé");
        for s in ["%", "%%", "%a", "%zz", "é%", "%é", "%%é", "a%2", "%%%%", "\u{1F600}%f"] {
            let _ = decode(s);
        }
        // Exhaustive: every 1-3 char combination of tricky characters.
        let alphabet = ['%', 'a', 'F', '0', 'é', '中', '+', 'g'];
        for &a in &alphabet {
            for &b in &alphabet {
                for &c in &alphabet {
                    let _ = decode(&format!("{a}{b}{c}"));
                    let _ = decode(&format!("%{a}{b}{c}"));
                }
            }
        }
    }

    #[test]
    fn non_loopback_requires_a_token() {
        let g = Guard::new("0.0.0.0", 7878, None);
        assert!(g.token().map_or(false, |t| t.len() == 32));
        assert_ne!(Guard::new("0.0.0.0", 1, None).token(), g.token());
        assert_eq!(Guard::new("0.0.0.0", 1, Some("s3cret".into())).token(), Some("s3cret"));
        assert!(Guard::new("127.0.0.1", 1, None).token().is_none());
        assert!(ct_eq(b"abc", b"abc") && !ct_eq(b"abc", b"abd") && !ct_eq(b"abc", b"ab"));
    }
}
