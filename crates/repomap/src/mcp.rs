//! The MCP server: repomap's tools on mcp-kit (rmcp over stdio).

use crate::tools;
use crate::workspace::Workspace;
use mcp_kit::roots::{self, Sources};
use mcp_kit::server::{run_stdio, App, Call, Info};
use serde_json::Value;
use std::path::PathBuf;

const INSTRUCTIONS: &str = "repomap is a map of this codebase. Start with `map` to see modules, central files and key symbols. Use `search` to find code by words or identifiers, `context` for a 360° view of a symbol or file (code, callers, callees, tests), `trace` for call paths, and `impact` before editing to see what could break (or `impact` with changed=true to review the current diff). `db` maps the database schema and where the code queries each table. Every answer cites file:line.";

/// Old launches (`@sylphx/locus`, `@sylphx/coderag`) set these.
const ROOT_ENV: &[&str] = &["REPOMAP_ROOT", "LOCUS_ROOT", "CODERAG_ROOT"];

struct Repomap {
    ws: Workspace,
    default_root: Option<PathBuf>,
    legacy: bool,
}

impl Repomap {
    fn root(&self, args: &Value, call: &Call) -> Result<PathBuf, String> {
        let explicit = ["root", "repo_root"].iter().find_map(|k| args.get(*k).and_then(|v| v.as_str())).map(PathBuf::from);
        roots::pick(&Sources { explicit, env: ROOT_ENV, default: self.default_root.clone(), client: &call.client_roots })
    }
}

impl App for Repomap {
    fn info(&self) -> Info {
        Info {
            name: "repomap".into(),
            title: "repomap".into(),
            version: crate::VERSION.into(),
            website: "https://sylphxai.github.io/repomap/".into(),
            instructions: INSTRUCTIONS.into(),
        }
    }

    fn tools(&self) -> Vec<Value> {
        tools::definitions(self.legacy)
    }

    fn call(&self, name: &str, args: &Value, call: &Call) -> Result<String, String> {
        let out = tools::call(&self.ws, name, args, &self.root(args, call)?)?;
        if args.get("format").and_then(|v| v.as_str()) == Some("json") {
            Ok(serde_json::to_string_pretty(&out.json).unwrap_or_default())
        } else {
            Ok(out.text)
        }
    }

    /// Index the project as soon as the client connects, so the first call is fast.
    fn warm(&self, call: &Call) {
        if let Ok(root) = self.root(&Value::Null, call) {
            let _ = self.ws.get(&root);
        }
    }
}

pub fn serve(default_root: Option<PathBuf>) -> anyhow::Result<()> {
    let legacy = std::env::var("REPOMAP_LEGACY_TOOLS").is_ok_and(|v| v == "1" || v == "true");
    run_stdio(Repomap { ws: Workspace::default(), default_root, legacy })
}
