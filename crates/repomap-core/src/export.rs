//! Compact graph payload for the web UI and static HTML export.

use crate::index::Index;
use serde_json::{json, Value};

pub struct ExportOptions {
    /// Include test files as nodes.
    pub include_tests: bool,
    /// Max symbols listed per file.
    pub symbols_per_file: usize,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self { include_tests: true, symbols_per_file: 120 }
    }
}

impl Index {
    pub fn graph_json(&self, opts: &ExportOptions, version: &str) -> Value {
        let mut ids: Vec<i64> = vec![-1; self.files.len()];
        let mut nodes = Vec::new();
        for (i, f) in self.code_files() {
            if !opts.include_tests && f.is_test {
                continue;
            }
            ids[i as usize] = nodes.len() as i64;
            let mut syms: Vec<usize> = self.file_symbols(i).collect();
            syms.sort_by(|a, b| self.sym_rank[*b].partial_cmp(&self.sym_rank[*a]).unwrap().then(a.cmp(b)));
            syms.truncate(opts.symbols_per_file);
            syms.sort_by_key(|s| self.symbols[*s].start);
            let symbols: Vec<Value> = syms
                .iter()
                .map(|&s| {
                    let sym = &self.symbols[s];
                    json!([sym.qualified(), sym.kind.as_str(), sym.start, sym.end, self.sym_in[s].len()])
                })
                .collect();
            nodes.push(json!({
                "p": f.path,
                "c": self.community.get(i as usize).copied().unwrap_or(0),
                "r": (self.file_rank[i as usize] * 10000.0).round() / 10000.0,
                "l": f.lines,
                "g": f.lang.map(|l| l.name()).unwrap_or(""),
                "t": f.is_test,
                "s": symbols,
            }));
        }
        let mut edges = Vec::new();
        for e in &self.file_edges {
            let (a, b) = (ids[e.from as usize], ids[e.to as usize]);
            if a < 0 || b < 0 {
                continue;
            }
            edges.push(json!([a, b, e.imports, e.calls]));
        }
        let communities: Vec<Value> = self
            .communities
            .iter()
            .map(|c| json!({ "id": c.id, "name": c.name, "size": c.files.len() }))
            .collect();
        json!({
            "version": version,
            "repo": self.root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            "commit": self.git.commit,
            "branch": self.git.branch,
            "web": self.git.remote_web,
            "stats": {
                "files": self.files.len(),
                "code_files": nodes.len(),
                "symbols": self.symbols.len(),
                "call_edges": self.sym_edges.len(),
                "file_edges": edges.len(),
                "index_ms": self.stats.total_ms,
            },
            "communities": communities,
            "nodes": nodes,
            "edges": edges,
        })
    }
}
