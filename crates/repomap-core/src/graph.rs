//! Linking: import resolution, call resolution, file graph, PageRank and
//! Louvain communities.

use crate::index::{Community, EdgeKind, FileEdge, Index, SymEdge};
use crate::lang::Lang;
use crate::parse::FileFacts;
use std::collections::{HashMap, HashSet};

pub fn link(index: &mut Index, facts: &[&FileFacts]) {
    let resolver = Resolver::new(index);
    let n = index.files.len();

    // 1. Imports -> file edges.
    let mut imports: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (i, f) in facts.iter().enumerate() {
        let Some(lang) = index.files[i].lang else { continue };
        let mut set: Vec<u32> = Vec::new();
        for spec in &f.imports {
            for t in resolver.resolve(i as u32, lang, spec) {
                if t != i as u32 && !set.contains(&t) {
                    set.push(t);
                }
            }
        }
        imports[i] = set;
    }

    // 2. Calls -> symbol edges.
    let mut sym_edges: Vec<SymEdge> = Vec::new();
    let mut file_calls: HashMap<(u32, u32), u32> = HashMap::new();
    for (i, f) in facts.iter().enumerate() {
        if f.calls.is_empty() {
            continue;
        }
        let entry = &index.files[i];
        let base = entry.sym_start;
        let imported: HashSet<u32> = imports[i].iter().copied().collect();
        let dir = parent_dir(&entry.path);
        let same_package = matches!(entry.lang, Some(Lang::Go) | Some(Lang::Java) | Some(Lang::CSharp) | Some(Lang::Rust));
        let mut seen: HashSet<(Option<u32>, u32)> = HashSet::new();
        for c in &f.calls {
            let Some(cands) = index.by_name.get(&c.name) else { continue };
            if cands.len() > 400 {
                continue;
            }
            let caller = c.from.map(|s| s + base);
            let ok = |s: &u32| {
                let sym = &index.symbols[*s as usize];
                if Some(*s) == caller {
                    return false;
                }
                if c.heritage {
                    sym.kind.is_container()
                } else {
                    sym.kind.is_callable() || matches!(sym.kind, crate::parse::Kind::Class | crate::parse::Kind::Struct)
                }
            };
            let cands: Vec<u32> = cands.iter().copied().filter(ok).collect();
            if cands.is_empty() {
                continue;
            }
            let chosen = choose_target(index, i as u32, caller, c, cands, &imported, dir, same_package);
            for to in chosen.into_iter().take(3) {
                let to_file = index.symbols[to as usize].file;
                if to_file != i as u32 {
                    *file_calls.entry((i as u32, to_file)).or_insert(0) += 1;
                }
                if let Some(from) = caller {
                    if seen.insert((Some(from), to)) {
                        sym_edges.push(SymEdge {
                            from,
                            to,
                            kind: if c.heritage { EdgeKind::Extends } else { EdgeKind::Calls },
                            line: c.line,
                        });
                    }
                }
            }
        }
    }

    // 3. Aggregate file edges.
    let mut agg: HashMap<(u32, u32), FileEdge> = HashMap::new();
    for (i, list) in imports.iter().enumerate() {
        for &t in list {
            agg.entry((i as u32, t))
                .or_insert(FileEdge { from: i as u32, to: t, imports: 0, calls: 0 })
                .imports += 1;
        }
    }
    for ((a, b), cnt) in file_calls {
        agg.entry((a, b)).or_insert(FileEdge { from: a, to: b, imports: 0, calls: 0 }).calls += cnt;
    }
    let mut file_edges: Vec<FileEdge> = agg.into_values().collect();
    file_edges.sort_by_key(|e| (e.from, e.to));

    let ns = index.symbols.len();
    let mut sym_out = vec![Vec::new(); ns];
    let mut sym_in = vec![Vec::new(); ns];
    for (k, e) in sym_edges.iter().enumerate() {
        sym_out[e.from as usize].push(k as u32);
        sym_in[e.to as usize].push(k as u32);
    }
    let mut file_out = vec![Vec::new(); n];
    let mut file_in = vec![Vec::new(); n];
    for (k, e) in file_edges.iter().enumerate() {
        file_out[e.from as usize].push(k as u32);
        file_in[e.to as usize].push(k as u32);
    }

    let file_rank = pagerank(n, &file_edges);
    let mut sym_rank = vec![0f32; ns];
    for e in &sym_edges {
        let from_file = index.symbols[e.from as usize].file;
        let to_file = index.symbols[e.to as usize].file;
        let w = if from_file == to_file { 0.2 } else { 1.0 };
        sym_rank[e.to as usize] += w * (file_rank[from_file as usize] + 1.0 / n.max(1) as f32);
    }
    for s in 0..ns {
        sym_rank[s] += file_rank[index.symbols[s].file as usize] * 0.05;
    }

    index.sym_edges = sym_edges;
    index.sym_out = sym_out;
    index.sym_in = sym_in;
    index.file_edges = file_edges;
    index.file_out = file_out;
    index.file_in = file_in;
    index.file_rank = file_rank;
    index.sym_rank = sym_rank;
    let (community, communities) = communities(index);
    index.community = community;
    index.communities = communities;
}

/// Method names too generic to resolve by global uniqueness alone.
const GENERIC_METHODS: &[&str] = &[
    "get", "set", "has", "add", "delete", "remove", "push", "pop", "map", "filter", "reduce", "find", "some",
    "every", "join", "split", "slice", "concat", "includes", "keys", "values", "entries", "then", "catch",
    "on", "off", "emit", "send", "write", "read", "close", "open", "start", "stop", "run", "call", "apply",
    "bind", "update", "clear", "next", "len", "iter", "unwrap", "expect", "clone", "into", "from", "new",
    "default", "append", "extend", "insert", "items", "format", "log", "error", "warn", "info", "debug",
    "init", "create", "load", "save", "parse", "render", "handle", "process", "execute", "build", "equals",
    "size", "length", "contains", "copy", "reset", "flush", "wait", "lock", "use", "each", "list", "sort",
    "test", "exec", "query", "fetch", "post", "put", "patch", "match", "replace", "trim", "lower", "upper",
];

fn choose_target(
    index: &Index,
    file: u32,
    caller: Option<u32>,
    c: &crate::parse::CallFact,
    cands: Vec<u32>,
    imported: &HashSet<u32>,
    dir: &str,
    same_package: bool,
) -> Vec<u32> {
    let sym = |s: &u32| &index.symbols[*s as usize];
    let local: Vec<u32> = cands.iter().copied().filter(|s| sym(s).file == file).collect();
    if c.member {
        let qual = c.qual.as_deref().unwrap_or("");
        if matches!(qual, "this" | "self" | "Self" | "cls" | "super" | "me") {
            let owner = caller.and_then(|s| index.symbols[s as usize].owner.clone());
            let owned: Vec<u32> = local.iter().copied().filter(|s| sym(s).owner == owner).collect();
            return if owned.is_empty() { local } else { owned };
        }
        if !qual.is_empty() {
            let by_owner: Vec<u32> = cands.iter().copied().filter(|s| sym(s).owner.as_deref() == Some(qual)).collect();
            if !by_owner.is_empty() {
                let near: Vec<u32> = by_owner.iter().copied().filter(|s| sym(s).file == file || imported.contains(&sym(s).file)).collect();
                return if near.is_empty() { by_owner } else { near };
            }
        }
        if !local.is_empty() {
            return local;
        }
        let via_import: Vec<u32> = cands.iter().copied().filter(|s| imported.contains(&sym(s).file)).collect();
        if !via_import.is_empty() && via_import.len() <= 3 {
            return via_import;
        }
        if cands.len() == 1 && c.name.len() >= 4 && !GENERIC_METHODS.contains(&c.name.as_str()) {
            return cands;
        }
        return Vec::new();
    }
    if !local.is_empty() {
        return local;
    }
    let via_import: Vec<u32> = cands.iter().copied().filter(|s| imported.contains(&sym(s).file)).collect();
    if !via_import.is_empty() {
        return via_import;
    }
    if cands.len() == 1 {
        return cands;
    }
    let same_dir: Vec<u32> = cands
        .iter()
        .copied()
        .filter(|s| parent_dir(&index.files[sym(s).file as usize].path) == dir)
        .collect();
    if same_dir.len() == 1 || (same_package && !same_dir.is_empty() && same_dir.len() <= 3) {
        same_dir
    } else {
        Vec::new()
    }
}

pub fn parent_dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

fn join(dir: &str, rel: &str) -> String {
    if dir.is_empty() {
        normalize(rel)
    } else {
        normalize(&format!("{dir}/{rel}"))
    }
}

struct Resolver<'a> {
    index: &'a Index,
    /// file name -> file ids
    by_filename: HashMap<&'a str, Vec<u32>>,
    /// path without extension -> file ids
    by_stem_path: HashMap<String, Vec<u32>>,
    /// dir -> code file ids
    by_dir: HashMap<&'a str, Vec<u32>>,
    /// npm workspace package name -> package dir
    js_packages: Vec<(String, String)>,
    /// Rust crate name (underscored) -> src dir
    crates: HashMap<String, String>,
}

fn strip_ext(path: &str) -> &str {
    let name_start = path.rfind('/').map_or(0, |i| i + 1);
    match path[name_start..].find('.') {
        Some(i) => &path[..name_start + i],
        None => path,
    }
}

impl<'a> Resolver<'a> {
    fn new(index: &'a Index) -> Self {
        let mut by_filename: HashMap<&str, Vec<u32>> = HashMap::new();
        let mut by_stem_path: HashMap<String, Vec<u32>> = HashMap::new();
        let mut by_dir: HashMap<&str, Vec<u32>> = HashMap::new();
        let mut js_packages = Vec::new();
        let mut crates = HashMap::new();
        for (i, f) in index.files.iter().enumerate() {
            let name = f.path.rsplit('/').next().unwrap_or(&f.path);
            by_filename.entry(name).or_default().push(i as u32);
            if f.lang.is_some() {
                by_stem_path.entry(strip_ext(&f.path).to_string()).or_default().push(i as u32);
                by_dir.entry(parent_dir(&f.path)).or_default().push(i as u32);
            }
            if name == "package.json" && !f.path.contains("node_modules/") {
                if let Ok(txt) = std::fs::read_to_string(index.root.join(&f.path)) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
                        if let Some(n) = v.get("name").and_then(|n| n.as_str()) {
                            js_packages.push((n.to_string(), parent_dir(&f.path).to_string()));
                        }
                    }
                }
            }
            if name == "Cargo.toml" {
                if let Ok(txt) = std::fs::read_to_string(index.root.join(&f.path)) {
                    let mut in_pkg = false;
                    for line in txt.lines() {
                        let l = line.trim();
                        if l.starts_with('[') {
                            in_pkg = l == "[package]";
                        } else if in_pkg && l.starts_with("name") {
                            if let Some(v) = l.split('"').nth(1) {
                                let dir = parent_dir(&f.path);
                                let src = if dir.is_empty() { "src".to_string() } else { format!("{dir}/src") };
                                crates.insert(v.replace('-', "_"), src);
                            }
                        }
                    }
                }
            }
        }
        js_packages.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        Resolver { index, by_filename, by_stem_path, by_dir, js_packages, crates }
    }

    fn file(&self, path: &str) -> Option<u32> {
        self.index.path_ix.get(path).copied().filter(|i| self.index.files[*i as usize].lang.is_some())
    }

    fn resolve(&self, from: u32, lang: Lang, spec: &str) -> Vec<u32> {
        let path = &self.index.files[from as usize].path;
        let dir = parent_dir(path);
        match lang {
            Lang::TypeScript | Lang::Tsx | Lang::JavaScript => self.resolve_js(dir, spec).into_iter().collect(),
            Lang::Python => self.resolve_python(from, dir, spec).into_iter().collect(),
            Lang::Go => self.resolve_go(spec),
            Lang::Rust => self.resolve_rust(path, spec).into_iter().collect(),
            Lang::Java | Lang::Php => self.resolve_qualified(dir, spec),
            Lang::C | Lang::Cpp => self.resolve_include(dir, spec).into_iter().collect(),
            Lang::Ruby => self.resolve_ruby(dir, spec).into_iter().collect(),
            Lang::CSharp => Vec::new(),
        }
    }

    fn try_js(&self, base: &str) -> Option<u32> {
        const EXTS: [&str; 9] = ["", ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".d.ts"];
        for e in EXTS {
            if let Some(f) = self.file(&format!("{base}{e}")) {
                return Some(f);
            }
        }
        // ESM TypeScript imports "./x.js" that point at "./x.ts".
        for (from, to) in [(".js", ".ts"), (".js", ".tsx"), (".jsx", ".tsx"), (".mjs", ".mts")] {
            if let Some(stem) = base.strip_suffix(from) {
                if let Some(f) = self.file(&format!("{stem}{to}")) {
                    return Some(f);
                }
            }
        }
        for idx in ["index.ts", "index.tsx", "index.js", "index.jsx", "index.mjs"] {
            let p = if base.is_empty() { idx.to_string() } else { format!("{base}/{idx}") };
            if let Some(f) = self.file(&p) {
                return Some(f);
            }
        }
        None
    }

    fn resolve_js(&self, dir: &str, spec: &str) -> Option<u32> {
        if spec.starts_with('.') {
            return self.try_js(&join(dir, spec));
        }
        if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
            // Common alias: the nearest ancestor `src/` of the importer, else root.
            let mut d = dir;
            loop {
                let cand = if d.is_empty() { format!("src/{rest}") } else { format!("{d}/src/{rest}") };
                if let Some(f) = self.try_js(&cand) {
                    return Some(f);
                }
                if d.ends_with("/src") || d == "src" {
                    if let Some(f) = self.try_js(&join(d, rest)) {
                        return Some(f);
                    }
                }
                if d.is_empty() {
                    break;
                }
                d = parent_dir(d);
            }
            return self.try_js(rest);
        }
        for (name, pdir) in &self.js_packages {
            if spec == name || spec.starts_with(&format!("{name}/")) {
                let sub = spec[name.len()..].trim_start_matches('/');
                let bases: Vec<String> = if sub.is_empty() {
                    vec![join(pdir, "src/index"), join(pdir, "index"), join(pdir, "src/main"), join(pdir, "src")]
                } else {
                    vec![join(pdir, &format!("src/{sub}")), join(pdir, sub)]
                };
                for b in bases {
                    if let Some(f) = self.try_js(&b) {
                        return Some(f);
                    }
                }
            }
        }
        None
    }

    fn resolve_python(&self, from: u32, dir: &str, spec: &str) -> Option<u32> {
        let try_mod = |base: &str| -> Option<u32> {
            self.file(&format!("{base}.py"))
                .or_else(|| self.file(&format!("{base}/__init__.py")))
                .or_else(|| self.file(&format!("{base}.pyi")))
        };
        if spec.starts_with('.') {
            let dots = spec.chars().take_while(|c| *c == '.').count();
            let mut d = dir.to_string();
            for _ in 1..dots {
                d = parent_dir(&d).to_string();
            }
            let rest = spec[dots..].replace('.', "/");
            if rest.is_empty() {
                return self.file(&join(&d, "__init__.py"));
            }
            return try_mod(&join(&d, &rest));
        }
        let rel = spec.replace('.', "/");
        // Search roots: repo root, src/, and every ancestor of the importer.
        let mut roots: Vec<String> = vec![String::new(), "src".into()];
        let mut d = dir.to_string();
        while !d.is_empty() {
            roots.push(d.clone());
            d = parent_dir(&d).to_string();
        }
        for r in &roots {
            if let Some(f) = try_mod(&join(r, &rel)) {
                return Some(f);
            }
        }
        // Fall back to a unique suffix match for multi-segment modules.
        if spec.contains('.') {
            let found = self.suffix_stem(&rel);
            if found.len() == 1 {
                return found.first().copied();
            }
            return pick_closest(self.index, from, &found);
        }
        None
    }

    fn suffix_stem(&self, rel: &str) -> Vec<u32> {
        let last = rel.rsplit('/').next().unwrap_or(rel);
        let mut out = Vec::new();
        for (name, ids) in &self.by_filename {
            if strip_ext(name) != last && *name != "__init__.py" {
                continue;
            }
            for &id in ids {
                let p = &self.index.files[id as usize].path;
                if self.index.files[id as usize].lang.is_none() {
                    continue;
                }
                let stem = strip_ext(p);
                let stem = stem.strip_suffix("/__init__").unwrap_or(stem);
                if stem == rel || stem.ends_with(&format!("/{rel}")) {
                    out.push(id);
                }
            }
        }
        out
    }

    fn resolve_go(&self, spec: &str) -> Vec<u32> {
        let segs: Vec<&str> = spec.split('/').collect();
        if segs.len() < 2 {
            return Vec::new();
        }
        for start in 0..segs.len() - 1 {
            let suffix = segs[start..].join("/");
            let mut matches: Vec<&str> = self
                .by_dir
                .keys()
                .copied()
                .filter(|d| *d == suffix || d.ends_with(&format!("/{suffix}")))
                .collect();
            if matches.is_empty() {
                continue;
            }
            matches.sort_by_key(|d| d.len());
            let d = matches[0];
            return self.by_dir[d]
                .iter()
                .copied()
                .filter(|f| {
                    let p = &self.index.files[*f as usize];
                    p.lang == Some(Lang::Go) && !p.is_test
                })
                .take(24)
                .collect();
        }
        Vec::new()
    }

    fn rust_module_dir(path: &str) -> String {
        let dir = parent_dir(path);
        let name = path.rsplit('/').next().unwrap_or(path);
        if matches!(name, "lib.rs" | "main.rs" | "mod.rs") {
            dir.to_string()
        } else {
            join(dir, strip_ext(name))
        }
    }

    fn crate_src(&self, path: &str) -> Option<String> {
        let mut d = parent_dir(path);
        loop {
            let src = if d.ends_with("/src") || d == "src" { Some(d.to_string()) } else { None };
            if let Some(s) = src {
                if self.file(&join(&s, "lib.rs")).is_some() || self.file(&join(&s, "main.rs")).is_some() {
                    return Some(s);
                }
            }
            if d.is_empty() {
                return None;
            }
            d = parent_dir(d);
        }
    }

    fn resolve_rust(&self, path: &str, spec: &str) -> Option<u32> {
        let try_mod = |base: &str| self.file(&format!("{base}.rs")).or_else(|| self.file(&format!("{base}/mod.rs")));
        if let Some(m) = spec.strip_prefix("mod:") {
            return try_mod(&join(&Self::rust_module_dir(path), m));
        }
        let head = spec.split("::{").next().unwrap_or(spec);
        let head = head.split(" as ").next().unwrap_or(head).trim();
        let segs: Vec<&str> = head.split("::").filter(|s| !s.is_empty() && *s != "*").collect();
        if segs.is_empty() {
            return None;
        }
        let (base, rest): (String, &[&str]) = match segs[0] {
            "crate" => (self.crate_src(path)?, &segs[1..]),
            "self" => (Self::rust_module_dir(path), &segs[1..]),
            "super" => {
                let mut d = Self::rust_module_dir(path);
                let mut i = 0;
                while i < segs.len() && segs[i] == "super" {
                    d = parent_dir(&d).to_string();
                    i += 1;
                }
                (d, &segs[i..])
            }
            name => (self.crates.get(name)?.clone(), &segs[1..]),
        };
        for k in (1..=rest.len()).rev() {
            if let Some(f) = try_mod(&join(&base, &rest[..k].join("/"))) {
                return Some(f);
            }
        }
        self.file(&join(&base, "lib.rs")).or_else(|| self.file(&join(&base, "main.rs")))
    }

    fn resolve_qualified(&self, dir: &str, spec: &str) -> Vec<u32> {
        let s = spec.trim().trim_start_matches('\\').trim_end_matches(';');
        let s = s.split(" as ").next().unwrap_or(s).trim();
        let segs: Vec<&str> = s.split(['.', '\\']).filter(|x| !x.is_empty()).collect();
        if segs.len() < 2 {
            return Vec::new();
        }
        if *segs.last().unwrap() == "*" {
            let pkg = segs[..segs.len() - 1].join("/");
            if let Some((d, ids)) = self.by_dir.iter().find(|(d, _)| **d == pkg || d.ends_with(&format!("/{pkg}"))) {
                let _ = d;
                return ids.iter().copied().take(24).collect();
            }
            return Vec::new();
        }
        for start in 0..segs.len() - 1 {
            let rel = segs[start..].join("/");
            if let Some(ids) = self.by_stem_path.get(&rel) {
                return ids.clone();
            }
            let found = self.suffix_stem(&rel);
            if !found.is_empty() {
                if found.len() == 1 {
                    return found;
                }
                let _ = dir;
                return found.into_iter().take(1).collect();
            }
        }
        Vec::new()
    }

    fn resolve_include(&self, dir: &str, spec: &str) -> Option<u32> {
        if let Some(f) = self.file(&join(dir, spec)) {
            return Some(f);
        }
        if let Some(f) = self.file(&normalize(spec)) {
            return Some(f);
        }
        let name = spec.rsplit('/').next().unwrap_or(spec);
        let ids = self.by_filename.get(name)?;
        let want = format!("/{}", normalize(spec));
        let cands: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|i| {
                let p = &self.index.files[*i as usize].path;
                p.ends_with(&want) || *p == normalize(spec)
            })
            .collect();
        match cands.len() {
            0 => None,
            1 => Some(cands[0]),
            _ => cands.into_iter().max_by_key(|i| common_prefix(dir, &self.index.files[*i as usize].path)),
        }
    }

    fn resolve_ruby(&self, dir: &str, spec: &str) -> Option<u32> {
        let spec = spec.trim_end_matches(".rb");
        if let Some(f) = self.file(&format!("{}.rb", join(dir, spec))) {
            return Some(f);
        }
        for root in ["lib", "app", ""] {
            if let Some(f) = self.file(&format!("{}.rb", join(root, spec))) {
                return Some(f);
            }
        }
        None
    }
}

fn common_prefix(a: &str, b: &str) -> usize {
    a.split('/').zip(b.split('/')).take_while(|(x, y)| x == y).count()
}

fn pick_closest(index: &Index, from: u32, cands: &[u32]) -> Option<u32> {
    let p = &index.files[from as usize].path;
    cands.iter().copied().max_by_key(|c| common_prefix(p, &index.files[*c as usize].path))
}

pub fn pagerank(n: usize, edges: &[FileEdge]) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    let d = 0.85f32;
    let mut out_w = vec![0f32; n];
    for e in edges {
        out_w[e.from as usize] += e.weight();
    }
    let mut rank = vec![1.0 / n as f32; n];
    for _ in 0..40 {
        let mut next = vec![0f32; n];
        let mut dangling = 0f32;
        for i in 0..n {
            if out_w[i] == 0.0 {
                dangling += rank[i];
            }
        }
        for e in edges {
            let f = e.from as usize;
            next[e.to as usize] += d * rank[f] * e.weight() / out_w[f];
        }
        let base = (1.0 - d) / n as f32 + d * dangling / n as f32;
        let mut delta = 0f32;
        for i in 0..n {
            let v = next[i] + base;
            delta += (v - rank[i]).abs();
            rank[i] = v;
        }
        if delta < 1e-6 {
            break;
        }
    }
    // Normalise so the top file is 1.0.
    let max = rank.iter().cloned().fold(0f32, f32::max).max(1e-12);
    rank.iter().map(|r| r / max).collect()
}

/// Louvain modularity communities over the undirected, weighted file graph.
pub fn louvain(n: usize, edges: &[(u32, u32, f32)]) -> Vec<u32> {
    let mut node_comm: Vec<u32> = (0..n as u32).collect();
    let mut cur_n = n;
    let mut cur_edges: Vec<(u32, u32, f32)> = edges.to_vec();
    let mut mapping: Vec<u32> = (0..n as u32).collect(); // original -> current node
    for _level in 0..10 {
        let mut adj: Vec<Vec<(u32, f32)>> = vec![Vec::new(); cur_n];
        let mut k = vec![0f64; cur_n];
        let mut m2 = 0f64;
        for &(a, b, w) in &cur_edges {
            let w = w as f64;
            adj[a as usize].push((b, w as f32));
            if a != b {
                adj[b as usize].push((a, w as f32));
            }
            k[a as usize] += w;
            k[b as usize] += w;
            m2 += 2.0 * w;
        }
        if m2 == 0.0 {
            break;
        }
        let mut comm: Vec<u32> = (0..cur_n as u32).collect();
        let mut tot: Vec<f64> = k.clone();
        let mut improved = false;
        for _pass in 0..20 {
            let mut moved = 0;
            for i in 0..cur_n {
                let ci = comm[i] as usize;
                let mut w_to: HashMap<u32, f64> = HashMap::new();
                for &(j, w) in &adj[i] {
                    if j as usize != i {
                        *w_to.entry(comm[j as usize]).or_insert(0.0) += w as f64;
                    }
                }
                tot[ci] -= k[i];
                let own = *w_to.get(&(ci as u32)).unwrap_or(&0.0);
                let mut best = ci as u32;
                let mut best_gain = own - tot[ci] * k[i] / m2;
                let mut keys: Vec<(&u32, &f64)> = w_to.iter().collect();
                keys.sort_by_key(|(c, _)| **c);
                for (&c, &w) in keys {
                    let gain = w - tot[c as usize] * k[i] / m2;
                    if gain > best_gain + 1e-12 {
                        best_gain = gain;
                        best = c;
                    }
                }
                tot[best as usize] += k[i];
                if best as usize != ci {
                    comm[i] = best;
                    moved += 1;
                    improved = true;
                }
            }
            if moved == 0 {
                break;
            }
        }
        if !improved {
            break;
        }
        // Renumber and aggregate.
        let mut renum: HashMap<u32, u32> = HashMap::new();
        for c in comm.iter_mut() {
            let len = renum.len() as u32;
            *c = *renum.entry(*c).or_insert(len);
        }
        for m in mapping.iter_mut() {
            *m = comm[*m as usize];
        }
        let mut agg: HashMap<(u32, u32), f32> = HashMap::new();
        for &(a, b, w) in &cur_edges {
            let (x, y) = (comm[a as usize], comm[b as usize]);
            let key = if x <= y { (x, y) } else { (y, x) };
            *agg.entry(key).or_insert(0.0) += w;
        }
        cur_n = renum.len();
        let mut next: Vec<(u32, u32, f32)> = agg.into_iter().map(|((a, b), w)| (a, b, w)).collect();
        next.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        cur_edges = next;
    }
    for (i, c) in node_comm.iter_mut().enumerate() {
        *c = mapping[i];
    }
    node_comm
}

fn communities(index: &Index) -> (Vec<u32>, Vec<Community>) {
    let n = index.files.len();
    let code: Vec<u32> = index.code_files().map(|(i, _)| i).collect();
    let mut local: HashMap<u32, u32> = HashMap::new();
    for (k, &f) in code.iter().enumerate() {
        local.insert(f, k as u32);
    }
    let mut und: HashMap<(u32, u32), f32> = HashMap::new();
    for e in &index.file_edges {
        let (Some(&a), Some(&b)) = (local.get(&e.from), local.get(&e.to)) else { continue };
        let key = if a <= b { (a, b) } else { (b, a) };
        *und.entry(key).or_insert(0.0) += e.weight();
    }
    let mut edges: Vec<(u32, u32, f32)> = und.into_iter().map(|((a, b), w)| (a, b, w)).collect();
    edges.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let raw = louvain(code.len(), &edges);

    // Group, then fold tiny groups into their directory's dominant community.
    let mut groups: HashMap<u32, Vec<u32>> = HashMap::new();
    for (k, &c) in raw.iter().enumerate() {
        groups.entry(c).or_default().push(code[k]);
    }
    let mut assign: HashMap<u32, u32> = HashMap::new();
    let mut big: Vec<Vec<u32>> = Vec::new();
    let mut small: Vec<u32> = Vec::new();
    let mut keys: Vec<u32> = groups.keys().copied().collect();
    keys.sort();
    for key in keys {
        let files = &groups[&key];
        if files.len() >= 3 {
            let id = big.len() as u32;
            for &f in files {
                assign.insert(f, id);
            }
            big.push(files.clone());
        } else {
            small.extend(files.iter().copied());
        }
    }
    // Directory vote for small groups.
    let mut dir_votes: HashMap<&str, HashMap<u32, u32>> = HashMap::new();
    for (&f, &c) in &assign {
        *dir_votes.entry(parent_dir(&index.files[f as usize].path)).or_default().entry(c).or_insert(0) += 1;
    }
    let mut dir_groups: HashMap<String, Vec<u32>> = HashMap::new();
    small.sort();
    for f in small {
        let dir = parent_dir(&index.files[f as usize].path);
        if let Some(v) = dir_votes.get(dir) {
            if let Some((&c, _)) = v.iter().max_by_key(|(c, n)| (**n, std::cmp::Reverse(**c))) {
                big[c as usize].push(f);
                assign.insert(f, c);
                continue;
            }
        }
        let top: String = dir.split('/').take(2).collect::<Vec<_>>().join("/");
        dir_groups.entry(top).or_default().push(f);
    }
    let mut dg: Vec<(String, Vec<u32>)> = dir_groups.into_iter().collect();
    dg.sort();
    for (_, files) in dg {
        big.push(files);
    }
    big.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));

    let mut community = vec![u32::MAX; n];
    let mut out = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    for (id, files) in big.into_iter().enumerate() {
        for &f in &files {
            community[f as usize] = id as u32;
        }
        let mut name = name_for(index, &files);
        let mut generic = is_generic_dir(name.rsplit('/').next().unwrap_or(&name));
        if generic && name.contains('/') {
            name = parent_dir(&name).to_string();
            generic = is_generic_dir(name.rsplit('/').next().unwrap_or(&name));
        }
        if generic || used.contains(&name) {
            let top = files
                .iter()
                .filter(|f| !index.files[**f as usize].is_test)
                .max_by(|a, b| index.file_rank[**a as usize].partial_cmp(&index.file_rank[**b as usize]).unwrap())
                .or_else(|| files.first())
                .map(|f| {
                    let p = &index.files[*f as usize].path;
                    let stem = strip_ext(p.rsplit('/').next().unwrap_or(p));
                    if matches!(stem, "index" | "mod" | "lib" | "main" | "__init__") {
                        let mut d = parent_dir(p);
                        while is_generic_dir(d.rsplit('/').next().unwrap_or(d)) && d.contains('/') {
                            d = parent_dir(d);
                        }
                        d.rsplit('/').next().unwrap_or(stem).to_string()
                    } else {
                        stem.to_string()
                    }
                })
                .unwrap_or_default();
            let candidate = format!("{name} · {top}");
            name = if used.contains(&candidate) { format!("{candidate} {id}") } else { candidate };
        }
        used.insert(name.clone());
        out.push(Community { id: id as u32, name, files });
    }
    (community, out)
}

fn is_generic_dir(d: &str) -> bool {
    matches!(d, "" | "src" | "lib" | "app" | "pkg" | "internal" | "source" | "packages" | "crates" | "(root)" | "tests" | "test")
}

fn name_for(index: &Index, all: &[u32]) -> String {
    let non_test: Vec<u32> = all.iter().copied().filter(|f| !index.files[*f as usize].is_test).collect();
    let files: &[u32] = if non_test.is_empty() { all } else { &non_test };
    // Rank-weighted directory coverage: central files decide the name.
    let weight = |f: u32| 0.05 + index.file_rank.get(f as usize).copied().unwrap_or(0.0);
    let total: f32 = files.iter().map(|&f| weight(f)).sum();
    let mut counts: HashMap<String, f32> = HashMap::new();
    for &f in files {
        let dir = parent_dir(&index.files[f as usize].path);
        let mut acc = String::new();
        for seg in dir.split('/').filter(|s| !s.is_empty()) {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(seg);
            *counts.entry(acc.clone()).or_insert(0.0) += weight(f);
        }
    }
    let best = counts
        .iter()
        .filter(|(_, c)| **c >= total * 0.5)
        .max_by(|(da, ca), (db, cb)| {
            (da.matches('/').count(), **ca).partial_cmp(&(db.matches('/').count(), **cb)).unwrap_or(std::cmp::Ordering::Equal).then(db.cmp(da))
        });
    match best {
        Some((d, _)) => d.clone(),
        None => {
            let top = files
                .iter()
                .max_by(|a, b| index.file_rank[**a as usize].partial_cmp(&index.file_rank[**b as usize]).unwrap())
                .copied();
            match top {
                Some(f) => {
                    let p = &index.files[f as usize].path;
                    let d = parent_dir(p);
                    if d.is_empty() { "(root)".into() } else { d.to_string() }
                }
                None => "(root)".into(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn louvain_splits_two_cliques() {
        let mut e = Vec::new();
        for a in 0..4u32 {
            for b in (a + 1)..4 {
                e.push((a, b, 1.0));
                e.push((a + 4, b + 4, 1.0));
            }
        }
        e.push((0, 4, 0.1));
        let c = louvain(8, &e);
        assert_eq!(c[0], c[3]);
        assert_eq!(c[4], c[7]);
        assert_ne!(c[0], c[4]);
    }

    #[test]
    fn normalizes_paths() {
        assert_eq!(join("src/a", "../b/c"), "src/b/c");
        assert_eq!(strip_ext("a/b.test.ts"), "a/b");
    }
}
