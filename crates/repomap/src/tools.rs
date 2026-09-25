//! Tool dispatch shared by the MCP server and the HTTP API.

use crate::workspace::Workspace;
use repomap_core::query::Direction;
use repomap_core::{ContextOptions, ImpactOptions, MapOptions, SearchOptions, TraceOptions};
use serde_json::{json, Value};


/// Legacy tool names from Spine and Locus, kept callable for one major.
pub fn canonical(name: &str) -> Option<&'static str> {
    Some(match name {
        "map" | "architecture_index" | "architecture_status" | "architecture_overview" | "architecture_explain" => "map",
        "search" | "architecture_search" | "codebase_search" => "search",
        "context" | "architecture_context_pack" | "architecture_evidence" | "find_related" => "context",
        "trace" | "architecture_path" | "architecture_trace" => "trace",
        "impact" | "architecture_impact" => "impact",
        "db" | "database" | "schema" => "db",
        _ => return None,
    })
}

pub fn definitions(include_legacy: bool) -> Vec<Value> {
    let root = json!({"type": "string", "description": "Repository root. Defaults to the client's workspace root or the server's working directory."});
    let format = json!({"type": "string", "enum": ["text", "json"], "description": "text (default, compact, file:line cited) or json."});
    let mut tools = vec![
        json!({
            "name": "map",
            "title": "Map the codebase",
            "description": "Start here. A map of the repository: modules (communities of files that depend on each other), the most central files (PageRank), the most used symbols, and entry points. Pass `focus` (a directory) to zoom in and get an outline of its files and symbols with line numbers.",
            "inputSchema": {"type": "object", "properties": {
                "focus": {"type": "string", "description": "Directory to zoom into, e.g. src/server."},
                "limit": {"type": "integer", "description": "Items per section (default 12)."},
                "root": root, "format": format
            }},
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "search",
            "title": "Search code",
            "description": "Hybrid code search: symbol names plus BM25 over AST chunks (whole functions, methods, classes). Use natural words or identifiers, e.g. \"refresh token expiry\" or \"parseConfig\". Returns ranked file:line ranges with the matching lines.",
            "inputSchema": {"type": "object", "required": ["query"], "properties": {
                "query": {"type": "string"},
                "limit": {"type": "integer", "description": "Max results (default 10)."},
                "path": {"type": "string", "description": "Only files whose path starts with or contains this."},
                "kind": {"type": "string", "enum": ["function", "method", "class", "struct", "interface", "trait", "enum", "type", "module", "macro"]},
                "include_tests": {"type": "boolean", "description": "Default true (tests rank lower)."},
                "root": root, "format": format
            }},
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "context",
            "title": "360° view of a symbol or file",
            "description": "Everything about one symbol or file: its code, who calls it (with call-site lines), what it calls, subtypes, members, imports, importers and the tests that touch it. Target forms: `path/to/file.ts`, `file.ts:42`, `Class.method`, `Class::method`, or a bare name.",
            "inputSchema": {"type": "object", "required": ["target"], "properties": {
                "target": {"type": "string"},
                "code_lines": {"type": "integer", "description": "Lines of source to include for a symbol (default 60, 0 for none)."},
                "root": root, "format": format
            }},
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "trace",
            "title": "Trace call paths",
            "description": "With `from` and `to`: the shortest call path between two symbols or files, each hop cited file:line (falls back to the file dependency path). With only `from`: the call tree below it (direction callees) or above it (direction callers).",
            "inputSchema": {"type": "object", "required": ["from"], "properties": {
                "from": {"type": "string"},
                "to": {"type": "string"},
                "direction": {"type": "string", "enum": ["callees", "callers"]},
                "depth": {"type": "integer", "description": "Tree depth (default 3)."},
                "root": root, "format": format
            }},
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "impact",
            "title": "Change impact (blast radius)",
            "description": "What breaks if this changes. Give `target` (symbol or file, or a list) or `changed: true` to analyse the current git diff. Returns a risk level, direct and indirect callers with call sites, importing files, modules touched, and the tests to run.",
            "inputSchema": {"type": "object", "properties": {
                "target": {"oneOf": [{"type": "string"}, {"type": "array", "items": {"type": "string"}}]},
                "changed": {"type": "boolean", "description": "Use the working-tree git diff (against `base`)."},
                "base": {"type": "string", "description": "Git ref for `changed` (default HEAD)."},
                "depth": {"type": "integer", "description": "Caller depth (default 3)."},
                "root": root, "format": format
            }},
            "annotations": {"readOnlyHint": true, "openWorldHint": false}
        }),
        json!({
            "name": "db",
            "title": "Database map",
            "description": "Map the database: tables, columns, primary and foreign keys, indexes, and the code that queries each table (file:line). By default it reads schema sources in the repo: SQL migrations, Prisma, Drizzle, SQLAlchemy and Diesel. To inspect a live Postgres, MySQL or SQLite database, pass `url_env` (the NAME of an environment variable holding the connection string). The connection is strictly read-only and the string is never stored or shown. Pass `table` for one table's full detail.",
            "inputSchema": {"type": "object", "properties": {
                "table": {"type": "string", "description": "Focus on one table: columns, indexes, who references it, and where the code queries it."},
                "url_env": {"type": "string", "description": "Name of an env var with the connection string, e.g. DATABASE_URL."},
                "url": {"type": "string", "description": "Connection string (prefer url_env so secrets stay out of transcripts)."},
                "root": root, "format": format
            }},
            "annotations": {"readOnlyHint": true, "openWorldHint": true}
        }),
    ];
    if include_legacy {
        for (old, new) in [
            ("architecture_overview", "map"),
            ("architecture_search", "search"),
            ("architecture_path", "trace"),
            ("architecture_impact", "impact"),
            ("architecture_context_pack", "context"),
            ("codebase_search", "search"),
        ] {
            tools.push(json!({
                "name": old,
                "description": format!("Deprecated alias of `{new}` (removed in repomap 2.0)."),
                "inputSchema": {"type": "object", "additionalProperties": true}
            }));
        }
    }
    tools
}

fn s<'a>(a: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| a.get(*k).and_then(|v| v.as_str())).filter(|v| !v.is_empty())
}

fn n(a: &Value, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|k| a.get(*k).and_then(|v| v.as_u64())).map(|v| v as usize)
}

fn strings(a: &Value, keys: &[&str]) -> Vec<String> {
    for k in keys {
        match a.get(*k) {
            Some(Value::String(v)) if !v.is_empty() => return vec![v.clone()],
            Some(Value::Array(v)) => {
                let out: Vec<String> = v.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                if !out.is_empty() {
                    return out;
                }
            }
            _ => {}
        }
    }
    Vec::new()
}

pub struct Output {
    pub text: String,
    pub json: Value,
}

/// Run a tool. `root` must already be resolved.
pub fn call(ws: &Workspace, name: &str, args: &Value, root: &std::path::Path) -> Result<Output, String> {
    let tool = canonical(name).ok_or_else(|| format!("unknown tool `{name}`"))?;
    let index = ws.get(root).map_err(|e| format!("indexing {} failed: {e}", root.display()))?;
    let index = &*index;
    macro_rules! out {
        ($r:expr) => {{
            let r = $r;
            Ok(Output { text: r.text(), json: serde_json::to_value(&r).unwrap_or(Value::Null) })
        }};
    }
    match tool {
        "map" => {
            let focus = s(args, &["focus", "path", "scope"]).map(String::from);
            out!(index.map(&MapOptions { focus, limit: n(args, &["limit"]).unwrap_or(12) }))
        }
        "search" => {
            let query = s(args, &["query", "q", "text"]).ok_or("`query` is required")?;
            let opts = SearchOptions {
                limit: n(args, &["limit"]).unwrap_or(10).clamp(1, 100),
                path: s(args, &["path", "path_filter"]).map(String::from),
                kind: s(args, &["kind"]).map(String::from),
                snippet_lines: if args.get("include_content").and_then(|v| v.as_bool()) == Some(false) { 0 } else { 4 },
                include_tests: args.get("include_tests").and_then(|v| v.as_bool()).unwrap_or(true),
            };
            out!(index.search(query, &opts))
        }
        "context" => {
            let target = match (s(args, &["target", "symbol", "node", "focus", "id", "file"]), s(args, &["path"]), n(args, &["line"])) {
                (Some(t), _, _) => t.to_string(),
                (None, Some(p), Some(l)) => format!("{p}:{l}"),
                (None, Some(p), None) => p.to_string(),
                _ => return Err("`target` is required".into()),
            };
            let opts = ContextOptions { code_lines: n(args, &["code_lines"]).unwrap_or(60), limit: n(args, &["limit"]).unwrap_or(25) };
            out!(index.context(&target, &opts)?)
        }
        "trace" => {
            let from = s(args, &["from", "source", "start", "symbol", "node", "target"]).ok_or("`from` is required")?;
            let to = if s(args, &["from", "source", "start"]).is_some() { s(args, &["to", "target", "end"]) } else { s(args, &["to", "end"]) };
            let direction = match s(args, &["direction"]) {
                Some("callers") | Some("up") | Some("in") | Some("incoming") => Direction::Callers,
                _ => Direction::Callees,
            };
            let opts = TraceOptions { direction, depth: n(args, &["depth", "max_depth"]).unwrap_or(3) };
            out!(index.trace(from, to, &opts)?)
        }
        "impact" => {
            let opts = ImpactOptions { depth: n(args, &["depth", "max_depth"]).unwrap_or(3), limit: n(args, &["limit"]).unwrap_or(30) };
            let targets = strings(args, &["target", "targets", "symbol", "paths", "changed_paths", "files"]);
            let changed = args.get("changed").and_then(|v| v.as_bool()).unwrap_or(false)
                || args.get("use_git_diff").and_then(|v| v.as_bool()).unwrap_or(false);
            if changed || targets.is_empty() {
                out!(index.impact_changed(s(args, &["base", "git_base"]), &opts)?)
            } else {
                out!(index.impact(&targets, &opts)?)
            }
        }
        "db" => {
            let url = match (s(args, &["url"]), s(args, &["url_env"])) {
                (Some(u), _) => Some(u.to_string()),
                (None, Some(var)) => Some(std::env::var(var).map_err(|_| format!("environment variable `{var}` is not set"))?),
                _ => None,
            };
            let schema = db_schema(index, url.as_deref()).map_err(|e| e.to_string())?;
            let table = s(args, &["table"]);
            let json = match table.and_then(|t| schema.table(t)) {
                Some(t) => serde_json::to_value(t).unwrap_or(Value::Null),
                None => serde_json::to_value(&schema).unwrap_or(Value::Null),
            };
            Ok(Output { text: schema.text(table), json })
        }
        _ => Err(format!("unknown tool `{name}`")),
    }
}

/// Live schema when a URL is given (with code names borrowed from the repo's
/// schema sources), otherwise the repo's own schema; then link tables to code.
pub fn db_schema(index: &repomap_core::Index, url: Option<&str>) -> anyhow::Result<repomap_core::db::DbSchema> {
    let from_repo = repomap_core::db::from_repo(index);
    let mut schema = match url {
        Some(u) => {
            let mut live = crate::dblive::introspect(u)?;
            for t in live.tables.iter_mut() {
                if let Some(st) = from_repo.table(&t.key()) {
                    t.aliases = st.aliases.clone();
                    t.source = format!("{} (defined in {})", t.source, st.source);
                }
            }
            live
        }
        None => from_repo,
    };
    repomap_core::db::link_code(index, &mut schema);
    Ok(schema)
}
