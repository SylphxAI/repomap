//! The five questions agents ask: map, search, context, trace, impact.
//! Each returns a serializable result with a compact text rendering that
//! cites `file:line`.

use crate::graph::parent_dir;
use crate::index::{EdgeKind, Index};
use crate::tokenize::tokenize;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    File(u32),
    Symbol(u32),
}

#[derive(Debug, Clone, Serialize)]
pub struct SymRef {
    pub name: String,
    pub kind: &'static str,
    pub file: String,
    pub line: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Site {
    pub symbol: SymRef,
    /// Call-site line (in the caller's file for callers, in this symbol for callees).
    pub at: u32,
    pub via: &'static str,
}

impl Index {
    pub fn sym_ref(&self, s: u32) -> SymRef {
        let sym = &self.symbols[s as usize];
        SymRef {
            name: sym.qualified(),
            kind: sym.kind.as_str(),
            file: self.files[sym.file as usize].path.clone(),
            line: sym.start,
            end: sym.end,
        }
    }

    fn community_name(&self, file: u32) -> &str {
        match self.community.get(file as usize) {
            Some(&c) if (c as usize) < self.communities.len() => &self.communities[c as usize].name,
            _ => "-",
        }
    }

    /// Resolve `path`, `path:line`, `Owner.name`, `Owner::name` or `name`.
    pub fn resolve(&self, q: &str) -> Result<(Target, Vec<Target>), String> {
        let q = q.trim().trim_start_matches("./");
        if q.is_empty() {
            return Err("empty target".into());
        }
        if let Some((p, line)) = q.rsplit_once(':') {
            if let Ok(line) = line.parse::<u32>() {
                if let Some(f) = self.find_file(p) {
                    return Ok((self.symbol_at(f, line).map_or(Target::File(f), Target::Symbol), vec![]));
                }
            }
        }
        if let Some(f) = self.find_file(q) {
            return Ok((Target::File(f), vec![]));
        }
        let parts: Vec<&str> = q.split(|c| c == '.' || c == '#' || c == ':').filter(|s| !s.is_empty()).collect();
        let (owner, name) = if parts.len() >= 2 {
            (Some(parts[parts.len() - 2]), parts[parts.len() - 1])
        } else {
            (None, q)
        };
        let mut cands: Vec<u32> = self.by_name.get(name).cloned().unwrap_or_default();
        if let Some(o) = owner {
            let owned: Vec<u32> = cands
                .iter()
                .copied()
                .filter(|s| self.symbols[*s as usize].owner.as_deref() == Some(o))
                .collect();
            if !owned.is_empty() {
                cands = owned;
            }
        }
        if cands.is_empty() {
            let lname = name.to_ascii_lowercase();
            cands = self
                .symbols
                .iter()
                .enumerate()
                .filter(|(_, s)| s.name.to_ascii_lowercase() == lname)
                .map(|(i, _)| i as u32)
                .collect();
        }
        if cands.is_empty() {
            return Err(format!("no file or symbol matches `{q}`. Try `search` first."));
        }
        cands.sort_by(|a, b| {
            let fa = self.files[self.symbols[*a as usize].file as usize].is_test;
            let fb = self.files[self.symbols[*b as usize].file as usize].is_test;
            fa.cmp(&fb).then(self.sym_rank[*b as usize].partial_cmp(&self.sym_rank[*a as usize]).unwrap_or(std::cmp::Ordering::Equal))
        });
        let best = Target::Symbol(cands[0]);
        let alts = cands[1..].iter().take(5).map(|s| Target::Symbol(*s)).collect();
        Ok((best, alts))
    }

    pub fn find_file(&self, p: &str) -> Option<u32> {
        let p = p.trim_start_matches("./").replace('\\', "/");
        if let Some(&f) = self.path_ix.get(&p) {
            return Some(f);
        }
        let abs_root = self.root.to_string_lossy().replace('\\', "/");
        if let Some(rel) = p.strip_prefix(&format!("{abs_root}/")) {
            if let Some(&f) = self.path_ix.get(rel) {
                return Some(f);
            }
        }
        if !p.contains('.') && !p.contains('/') {
            return None;
        }
        let suffix = format!("/{p}");
        let mut found = self.files.iter().enumerate().filter(|(_, f)| f.path.ends_with(&suffix));
        let first = found.next();
        if found.next().is_none() {
            return first.map(|(i, _)| i as u32);
        }
        None
    }

    fn target_label(&self, t: Target) -> String {
        match t {
            Target::File(f) => self.files[f as usize].path.clone(),
            Target::Symbol(s) => {
                let r = self.sym_ref(s);
                format!("{} {} ({}:{})", r.kind, r.name, r.file, r.line)
            }
        }
    }
}

// ---------------------------------------------------------------- map

#[derive(Debug, Serialize)]
pub struct MapResult {
    pub root: String,
    pub files: usize,
    pub code_files: usize,
    pub symbols: usize,
    pub call_edges: usize,
    pub file_edges: usize,
    pub languages: Vec<(String, usize)>,
    pub index_ms: u128,
    pub parsed: usize,
    pub cached: usize,
    pub modules: Vec<ModuleSummary>,
    pub key_files: Vec<KeyFile>,
    pub key_symbols: Vec<KeySymbol>,
    pub entry_points: Vec<String>,
    pub focus: Option<String>,
    pub outline: Vec<Outline>,
}

#[derive(Debug, Serialize)]
pub struct ModuleSummary {
    pub name: String,
    pub files: usize,
    pub key_files: Vec<String>,
    pub key_symbols: Vec<String>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct KeyFile {
    pub path: String,
    pub rank: f32,
    pub symbols: usize,
    pub imported_by: usize,
}

#[derive(Debug, Serialize)]
pub struct KeySymbol {
    pub symbol: SymRef,
    pub callers: usize,
}

#[derive(Debug, Serialize)]
pub struct Outline {
    pub path: String,
    pub symbols: Vec<(String, &'static str, u32, String)>,
}

pub struct MapOptions {
    pub focus: Option<String>,
    pub limit: usize,
}

impl Index {
    pub fn map(&self, opts: &MapOptions) -> MapResult {
        let focus = opts.focus.as_deref().map(|f| f.trim_matches('/').to_string()).filter(|f| !f.is_empty() && f != ".");
        let in_focus = |path: &str| match &focus {
            Some(f) => path == f || path.starts_with(&format!("{f}/")),
            None => true,
        };
        let code: Vec<u32> = self.code_files().filter(|(_, f)| in_focus(&f.path)).map(|(i, _)| i).collect();
        let mut langs: HashMap<&str, usize> = HashMap::new();
        for &f in &code {
            *langs.entry(self.files[f as usize].lang.unwrap().name()).or_insert(0) += 1;
        }
        let mut languages: Vec<(String, usize)> = langs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        languages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        let limit = opts.limit.max(3);
        let mut ranked = code.clone();
        ranked.sort_by(|a, b| self.file_rank[*b as usize].partial_cmp(&self.file_rank[*a as usize]).unwrap());
        let non_test: Vec<u32> = ranked.iter().copied().filter(|f| !self.files[*f as usize].is_test).collect();
        let key_files: Vec<KeyFile> = non_test
            .iter()
            .take(limit)
            .map(|&f| KeyFile {
                path: self.files[f as usize].path.clone(),
                rank: (self.file_rank[f as usize] * 1000.0).round() / 1000.0,
                symbols: self.file_symbols(f).len(),
                imported_by: self.file_in[f as usize].len(),
            })
            .collect();

        let focus_files: HashSet<u32> = code.iter().copied().collect();
        let mut syms: Vec<u32> = (0..self.symbols.len() as u32)
            .filter(|s| {
                let f = self.symbols[*s as usize].file;
                focus_files.contains(&f) && !self.files[f as usize].is_test
            })
            .collect();
        syms.sort_by(|a, b| self.sym_rank[*b as usize].partial_cmp(&self.sym_rank[*a as usize]).unwrap());
        let key_symbols: Vec<KeySymbol> = syms
            .iter()
            .take(limit)
            .map(|&s| KeySymbol { symbol: self.sym_ref(s), callers: self.sym_in[s as usize].len() })
            .collect();

        // Modules: communities intersecting the focus.
        let mut modules = Vec::new();
        for c in &self.communities {
            let files: Vec<u32> = c.files.iter().copied().filter(|f| focus_files.contains(f)).collect();
            if files.is_empty() {
                continue;
            }
            let mut fs = files.clone();
            fs.sort_by(|a, b| self.file_rank[*b as usize].partial_cmp(&self.file_rank[*a as usize]).unwrap());
            let key_files: Vec<String> = fs.iter().take(4).map(|f| self.files[*f as usize].path.clone()).collect();
            let fset: HashSet<u32> = files.iter().copied().collect();
            let mut ss: Vec<u32> = fs.iter().flat_map(|f| self.file_symbols(*f).map(|s| s as u32)).collect();
            ss.sort_by(|a, b| self.sym_rank[*b as usize].partial_cmp(&self.sym_rank[*a as usize]).unwrap());
            let key_symbols: Vec<String> = ss.iter().take(5).map(|s| self.symbols[*s as usize].qualified()).collect();
            let mut deps: HashMap<u32, f32> = HashMap::new();
            for &f in &files {
                for &e in &self.file_out[f as usize] {
                    let e = &self.file_edges[e as usize];
                    if !fset.contains(&e.to) {
                        let c = self.community[e.to as usize];
                        if c != u32::MAX {
                            *deps.entry(c).or_insert(0.0) += e.weight();
                        }
                    }
                }
            }
            let mut deps: Vec<(u32, f32)> = deps.into_iter().collect();
            deps.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
            modules.push(ModuleSummary {
                name: c.name.clone(),
                files: files.len(),
                key_files,
                key_symbols,
                depends_on: deps.iter().take(4).map(|(c, _)| self.communities[*c as usize].name.clone()).collect(),
            });
        }
        modules.truncate(limit);

        let entry_points: Vec<String> = code
            .iter()
            .copied()
            .filter(|&f| {
                let p = &self.files[f as usize].path;
                let name = p.rsplit('/').next().unwrap_or(p);
                let stem = name.split('.').next().unwrap_or(name);
                !self.files[f as usize].is_test
                    && matches!(stem, "main" | "index" | "cli" | "app" | "server" | "lib" | "__main__" | "manage" | "program" | "Program")
                    && parent_dir(p).split('/').count() <= 3
            })
            .take(limit.min(8))
            .map(|f| self.files[f as usize].path.clone())
            .collect();

        let mut outline = Vec::new();
        if focus.is_some() {
            for &f in ranked.iter().take(limit.max(20)) {
                let mut ss: Vec<usize> = self.file_symbols(f).filter(|s| self.symbols[*s].parent.map_or(true, |p| self.symbols[p as usize].kind.is_container())).collect();
                ss.truncate(30);
                outline.push(Outline {
                    path: self.files[f as usize].path.clone(),
                    symbols: ss
                        .into_iter()
                        .map(|s| {
                            let sym = &self.symbols[s];
                            (sym.qualified(), sym.kind.as_str(), sym.start, sym.signature.clone())
                        })
                        .collect(),
                });
            }
        }

        MapResult {
            root: self.root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            files: self.files.len(),
            code_files: code.len(),
            symbols: self.symbols.len(),
            call_edges: self.sym_edges.len(),
            file_edges: self.file_edges.len(),
            languages,
            index_ms: self.stats.total_ms,
            parsed: self.stats.files_parsed,
            cached: self.stats.files_cached,
            modules,
            key_files,
            key_symbols,
            entry_points,
            focus,
            outline,
        }
    }
}

impl MapResult {
    pub fn text(&self) -> String {
        let mut o = String::new();
        let scope = self.focus.as_deref().map(|f| format!(" / {f}")).unwrap_or_default();
        let _ = writeln!(o, "# Map of {}{}", self.root, scope);
        let langs: Vec<String> = self.languages.iter().take(6).map(|(l, n)| format!("{l} {n}")).collect();
        let _ = writeln!(
            o,
            "{} code files ({}), {} symbols, {} call edges, {} file dependencies. Indexed in {} ms ({} parsed, {} cached).",
            self.code_files,
            langs.join(", "),
            self.symbols,
            self.call_edges,
            self.file_edges,
            self.index_ms,
            self.parsed,
            self.cached
        );
        if !self.entry_points.is_empty() {
            let _ = writeln!(o, "Entry points: {}", self.entry_points.join(", "));
        }
        let _ = writeln!(o, "\n## Modules");
        for m in &self.modules {
            let _ = write!(o, "- **{}** ({} files): {}", m.name, m.files, m.key_files.join(", "));
            if !m.key_symbols.is_empty() {
                let _ = write!(o, " | key: {}", m.key_symbols.join(", "));
            }
            if !m.depends_on.is_empty() {
                let _ = write!(o, " | uses: {}", m.depends_on.join(", "));
            }
            o.push('\n');
        }
        let _ = writeln!(o, "\n## Most central files");
        for f in &self.key_files {
            let _ = writeln!(o, "- {} ({} symbols, imported by {})", f.path, f.symbols, f.imported_by);
        }
        let _ = writeln!(o, "\n## Most used symbols");
        for s in &self.key_symbols {
            let _ = writeln!(o, "- {} {} — {}:{} ({} callers)", s.symbol.kind, s.symbol.name, s.symbol.file, s.symbol.line, s.callers);
        }
        if !self.outline.is_empty() {
            let _ = writeln!(o, "\n## Outline");
            for f in &self.outline {
                let _ = writeln!(o, "{}", f.path);
                for (name, kind, line, sig) in &f.symbols {
                    let _ = writeln!(o, "  {line:>5}  {kind} {name}  `{}`", truncate(sig, 100));
                }
            }
        }
        o
    }
}

// ---------------------------------------------------------------- search

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub file: String,
    pub start: u32,
    pub end: u32,
    pub symbol: Option<SymRef>,
    pub score: f32,
    pub snippet: Vec<(u32, String)>,
    pub matched: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub hits: Vec<SearchHit>,
}

pub struct SearchOptions {
    pub limit: usize,
    pub path: Option<String>,
    pub kind: Option<String>,
    pub snippet_lines: usize,
    pub include_tests: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self { limit: 10, path: None, kind: None, snippet_lines: 4, include_tests: true }
    }
}

impl Index {
    pub fn search(&self, query: &str, opts: &SearchOptions) -> SearchResult {
        let path_ok = |file: u32| -> bool {
            let f = &self.files[file as usize];
            if !opts.include_tests && f.is_test {
                return false;
            }
            match &opts.path {
                Some(p) => f.path.starts_with(p.trim_start_matches("./")) || f.path.contains(p.as_str()),
                None => true,
            }
        };
        let kind_ok = |sym: Option<u32>| -> bool {
            match (&opts.kind, sym) {
                (None, _) => true,
                (Some(k), Some(s)) => self.symbols[s as usize].kind.as_str() == k,
                (Some(_), None) => false,
            }
        };

        // Lexical BM25 over chunks.
        let bm = self.bm25.search(query, 60, |c| path_ok(c.file) && kind_ok(c.symbol));

        // Symbol-name matches.
        let q = query.trim();
        let ql = q.to_ascii_lowercase();
        let qtoks: Vec<String> = tokenize(q);
        let single_word = !q.contains(char::is_whitespace);
        let mut sym_scores: Vec<(u32, f32)> = Vec::new();
        if !ql.is_empty() {
            for (i, s) in self.symbols.iter().enumerate() {
                let nl = self.names_lower[i].as_str();
                let score = if nl == ql || (s.owner.is_some() && ql.contains('.') && s.qualified().to_ascii_lowercase() == ql) {
                    3.0
                } else if single_word && nl.starts_with(&ql) && ql.len() >= 3 {
                    2.0
                } else if single_word && ql.len() >= 4 && nl.contains(&ql) {
                    1.5
                } else if qtoks.len() > 1 {
                    let ntoks = tokenize(&s.name);
                    let hit = qtoks.iter().filter(|t| ntoks.contains(t)).count();
                    if hit >= 2 { hit as f32 / qtoks.len() as f32 } else { 0.0 }
                } else {
                    0.0
                };
                if score > 0.0 && path_ok(s.file) && kind_ok(Some(i as u32)) {
                    sym_scores.push((i as u32, score + self.sym_rank[i] * 0.5));
                }
            }
        }
        sym_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        sym_scores.truncate(60);

        // Reciprocal-rank fusion keyed by (file, start line).
        const K: f32 = 20.0;
        let mut fused: HashMap<(u32, u32), (f32, u32, Option<u32>, Vec<String>)> = HashMap::new();
        for (rank, h) in bm.iter().enumerate() {
            let c = &self.bm25.chunks[h.chunk as usize];
            let e = fused.entry((c.file, c.start)).or_insert((0.0, c.end, c.symbol, Vec::new()));
            e.0 += 1.0 / (K + rank as f32);
            e.3 = h.matched.clone();
        }
        for (rank, (s, score)) in sym_scores.iter().enumerate() {
            let sym = &self.symbols[*s as usize];
            let weight = if *score >= 3.0 { 2.0 } else { 1.0 };
            let e = fused.entry((sym.file, sym.start)).or_insert((0.0, sym.end, Some(*s), Vec::new()));
            e.0 += weight / (K + rank as f32);
            if e.2.is_none() {
                e.2 = Some(*s);
            }
        }
        let mut merged: Vec<((u32, u32), (f32, u32, Option<u32>, Vec<String>))> = fused.into_iter().collect();
        for (k, v) in merged.iter_mut() {
            let f = &self.files[k.0 as usize];
            if f.is_test {
                v.0 *= 0.8;
            }
            if f.lang.is_none() {
                v.0 *= 0.85;
            }
            v.0 *= 1.0 + 0.15 * self.file_rank[k.0 as usize];
        }
        merged.sort_by(|a, b| b.1 .0.partial_cmp(&a.1 .0).unwrap().then(a.0.cmp(&b.0)));
        merged.truncate(opts.limit);

        let qset: HashSet<String> = qtoks.iter().cloned().collect();
        let hits = merged
            .into_iter()
            .map(|((file, start), (score, end, sym, matched))| {
                let snippet = self.snippet(file, start, end, &qset, opts.snippet_lines);
                SearchHit {
                    file: self.files[file as usize].path.clone(),
                    start,
                    end,
                    symbol: sym.map(|s| self.sym_ref(s)),
                    score: (score * 1000.0).round() / 1000.0,
                    snippet,
                    matched,
                }
            })
            .collect();
        SearchResult { query: query.to_string(), hits }
    }

    /// The first line plus the lines that best match the query terms.
    fn snippet(&self, file: u32, start: u32, end: u32, terms: &HashSet<String>, n: usize) -> Vec<(u32, String)> {
        if n == 0 {
            return Vec::new();
        }
        let Some(body) = self.read_lines(file, start, end) else { return Vec::new() };
        let lines: Vec<&str> = body.lines().collect();
        let first = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
        let mut scored: Vec<(usize, usize)> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != first)
            .map(|(i, l)| (i, tokenize(l).iter().filter(|t| terms.contains(*t)).count()))
            .filter(|(_, s)| *s > 0)
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut pick: Vec<usize> = vec![first];
        pick.extend(scored.iter().take(n.saturating_sub(1)).map(|(i, _)| *i));
        if pick.len() < n {
            for i in first + 1..lines.len() {
                if pick.len() >= n {
                    break;
                }
                if !pick.contains(&i) && !lines[i].trim().is_empty() {
                    pick.push(i);
                }
            }
        }
        pick.sort();
        pick.dedup();
        pick.into_iter()
            .filter(|i| *i < lines.len())
            .map(|i| (start + i as u32, truncate(lines[i].trim_end(), 160)))
            .collect()
    }
}

impl SearchResult {
    pub fn text(&self) -> String {
        let mut o = String::new();
        if self.hits.is_empty() {
            let _ = writeln!(o, "No matches for `{}`.", self.query);
            return o;
        }
        let _ = writeln!(o, "# Search: {}", self.query);
        for (i, h) in self.hits.iter().enumerate() {
            let label = match &h.symbol {
                Some(s) => format!("{} {}", s.kind, s.name),
                None => String::from("(text)"),
            };
            let _ = writeln!(o, "{}. {}:{}-{}  {}", i + 1, h.file, h.start, h.end, label);
            let mut last = 0;
            for (line, text) in &h.snippet {
                if last != 0 && *line > last + 1 {
                    let _ = writeln!(o, "      ...");
                }
                let _ = writeln!(o, "  {line:>5}| {text}");
                last = *line;
            }
        }
        o
    }
}

// ---------------------------------------------------------------- context

#[derive(Debug, Serialize)]
pub struct ContextResult {
    pub target: String,
    pub module: String,
    pub symbol: Option<SymRef>,
    pub signature: Option<String>,
    pub code: Option<String>,
    pub callers: Vec<Site>,
    pub callees: Vec<Site>,
    pub subtypes: Vec<SymRef>,
    pub members: Vec<SymRef>,
    pub imports: Vec<String>,
    pub imported_by: Vec<String>,
    pub tests: Vec<String>,
    pub alternatives: Vec<String>,
    pub lines: Option<u32>,
    pub language: Option<String>,
}

pub struct ContextOptions {
    pub code_lines: usize,
    pub limit: usize,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self { code_lines: 60, limit: 25 }
    }
}

impl Index {
    pub fn context(&self, target: &str, opts: &ContextOptions) -> Result<ContextResult, String> {
        let (t, alts) = self.resolve(target)?;
        let alternatives = alts.into_iter().map(|a| self.target_label(a)).collect();
        match t {
            Target::Symbol(s) => Ok(self.symbol_context(s, alternatives, opts)),
            Target::File(f) => Ok(self.file_context(f, alternatives, opts)),
        }
    }

    fn symbol_context(&self, s: u32, alternatives: Vec<String>, opts: &ContextOptions) -> ContextResult {
        let sym = &self.symbols[s as usize];
        let mut callers: Vec<Site> = Vec::new();
        let mut subtypes = Vec::new();
        for &e in &self.sym_in[s as usize] {
            let e = &self.sym_edges[e as usize];
            match e.kind {
                EdgeKind::Calls => callers.push(Site { symbol: self.sym_ref(e.from), at: e.line, via: "call" }),
                EdgeKind::Extends => subtypes.push(self.sym_ref(e.from)),
            }
        }
        // Include callers of the members for containers (class-level usage).
        let callees: Vec<Site> = self.sym_out[s as usize]
            .iter()
            .map(|&e| {
                let e = &self.sym_edges[e as usize];
                Site {
                    symbol: self.sym_ref(e.to),
                    at: e.line,
                    via: if e.kind == EdgeKind::Extends { "extends" } else { "call" },
                }
            })
            .collect();
        let members: Vec<SymRef> = self
            .file_symbols(sym.file)
            .filter(|i| self.symbols[*i].parent == Some(s))
            .map(|i| self.sym_ref(i as u32))
            .collect();
        let code = if opts.code_lines > 0 {
            let end = sym.end.min(sym.start + opts.code_lines as u32 - 1);
            self.read_lines(sym.file, sym.start, end).map(|c| {
                if end < sym.end {
                    format!("{c}\n... ({} more lines)", sym.end - end)
                } else {
                    c
                }
            })
        } else {
            None
        };
        let tests = self.tests_calling(s);
        let mut callers = callers;
        callers.sort_by(|a, b| (a.symbol.file.as_str(), a.at).cmp(&(b.symbol.file.as_str(), b.at)));
        callers.truncate(opts.limit);
        let mut callees = callees;
        callees.truncate(opts.limit);
        ContextResult {
            target: self.target_label(Target::Symbol(s)),
            module: self.community_name(sym.file).to_string(),
            symbol: Some(self.sym_ref(s)),
            signature: Some(sym.signature.clone()),
            code,
            callers,
            callees,
            subtypes,
            members,
            imports: Vec::new(),
            imported_by: Vec::new(),
            tests,
            alternatives,
            lines: None,
            language: self.files[sym.file as usize].lang.map(|l| l.name().to_string()),
        }
    }

    fn file_context(&self, f: u32, alternatives: Vec<String>, opts: &ContextOptions) -> ContextResult {
        let file = &self.files[f as usize];
        let imports: Vec<String> = self.file_out[f as usize]
            .iter()
            .map(|&e| self.files[self.file_edges[e as usize].to as usize].path.clone())
            .collect();
        let mut imported_by: Vec<(String, f32)> = self.file_in[f as usize]
            .iter()
            .map(|&e| {
                let e = &self.file_edges[e as usize];
                (self.files[e.from as usize].path.clone(), e.weight())
            })
            .collect();
        imported_by.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        let members: Vec<SymRef> = self
            .file_symbols(f)
            .filter(|i| self.symbols[*i].parent.map_or(true, |p| self.symbols[p as usize].kind.is_container()))
            .map(|i| self.sym_ref(i as u32))
            .take(opts.limit * 4)
            .collect();
        let mut callers: Vec<Site> = Vec::new();
        for s in self.file_symbols(f) {
            for &e in &self.sym_in[s] {
                let e = &self.sym_edges[e as usize];
                if self.symbols[e.from as usize].file != f {
                    callers.push(Site { symbol: self.sym_ref(e.from), at: e.line, via: "call" });
                }
            }
        }
        callers.sort_by(|a, b| (a.symbol.file.as_str(), a.at).cmp(&(b.symbol.file.as_str(), b.at)));
        callers.dedup_by(|a, b| a.symbol.name == b.symbol.name && a.symbol.file == b.symbol.file);
        callers.truncate(opts.limit);
        let syms: Vec<u32> = self.file_symbols(f).map(|s| s as u32).collect();
        ContextResult {
            target: file.path.clone(),
            module: self.community_name(f).to_string(),
            symbol: None,
            signature: None,
            code: None,
            callers,
            callees: Vec::new(),
            subtypes: Vec::new(),
            members,
            imports: imports.into_iter().take(opts.limit).collect(),
            imported_by: imported_by.into_iter().take(opts.limit).map(|(p, _)| p).collect(),
            tests: self.tests_for(&[f], &syms),
            alternatives,
            lines: Some(file.lines),
            language: file.lang.map(|l| l.name().to_string()),
        }
    }

    /// Test files whose symbols reach `s` within three call hops.
    fn tests_calling(&self, s: u32) -> Vec<String> {
        let mut seen: HashSet<u32> = HashSet::from([s]);
        let mut frontier = vec![s];
        let mut out: Vec<String> = Vec::new();
        for _ in 0..3 {
            let mut next = Vec::new();
            for u in frontier {
                for &e in &self.sym_in[u as usize] {
                    let from = self.sym_edges[e as usize].from;
                    if seen.insert(from) {
                        next.push(from);
                        let f = &self.files[self.symbols[from as usize].file as usize];
                        if f.is_test && !out.contains(&f.path) {
                            out.push(f.path.clone());
                        }
                    }
                }
            }
            frontier = next;
        }
        out.sort();
        out.truncate(20);
        out
    }

    /// Test files that import the files or call the symbols.
    fn tests_for(&self, files: &[u32], syms: &[u32]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut push = |f: u32| {
            let file = &self.files[f as usize];
            if file.is_test && !out.contains(&file.path) {
                out.push(file.path.clone());
            }
        };
        for &f in files {
            for &e in &self.file_in[f as usize] {
                push(self.file_edges[e as usize].from);
            }
        }
        for &s in syms {
            for &e in &self.sym_in[s as usize] {
                push(self.symbols[self.sym_edges[e as usize].from as usize].file);
            }
        }
        out.sort();
        out.truncate(20);
        out
    }
}

impl ContextResult {
    pub fn text(&self) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "# {}", self.target);
        let mut meta = vec![format!("module: {}", self.module)];
        if let Some(l) = &self.language {
            meta.push(l.clone());
        }
        if let Some(n) = self.lines {
            meta.push(format!("{n} lines"));
        }
        let _ = writeln!(o, "{}", meta.join(" · "));
        if !self.alternatives.is_empty() {
            let _ = writeln!(o, "Also matches: {}", self.alternatives.join("; "));
        }
        if let (Some(code), Some(sym)) = (&self.code, &self.symbol) {
            let _ = writeln!(o, "\n```\n{}\n```", number(code, sym.line));
        }
        let site = |o: &mut String, title: &str, v: &[Site], caller: bool| {
            if v.is_empty() {
                return;
            }
            let _ = writeln!(o, "\n## {} ({})", title, v.len());
            for s in v {
                if caller {
                    let _ = writeln!(o, "- {} {} — {}:{}", s.symbol.kind, s.symbol.name, s.symbol.file, s.at);
                } else {
                    let _ = writeln!(o, "- {} {} — {}:{} (at line {})", s.symbol.kind, s.symbol.name, s.symbol.file, s.symbol.line, s.at);
                }
            }
        };
        site(&mut o, "Called by", &self.callers, true);
        site(&mut o, "Calls", &self.callees, false);
        let list = |o: &mut String, title: &str, v: &[SymRef]| {
            if v.is_empty() {
                return;
            }
            let _ = writeln!(o, "\n## {} ({})", title, v.len());
            for s in v {
                let _ = writeln!(o, "- {} {} — {}:{}", s.kind, s.name, s.file, s.line);
            }
        };
        list(&mut o, "Subtypes / implementations", &self.subtypes);
        list(&mut o, if self.symbol.is_some() { "Members" } else { "Symbols" }, &self.members);
        let paths = |o: &mut String, title: &str, v: &[String]| {
            if v.is_empty() {
                return;
            }
            let _ = writeln!(o, "\n## {} ({})", title, v.len());
            for p in v {
                let _ = writeln!(o, "- {p}");
            }
        };
        paths(&mut o, "Imports", &self.imports);
        paths(&mut o, "Imported by", &self.imported_by);
        paths(&mut o, "Tests", &self.tests);
        o
    }
}

// ---------------------------------------------------------------- trace

#[derive(Debug, Serialize)]
pub struct Hop {
    pub from: String,
    pub to: String,
    pub via: &'static str,
    /// `file:line` of the call site or import.
    pub at: String,
}

#[derive(Debug, Serialize)]
pub struct TraceResult {
    pub from: String,
    pub to: Option<String>,
    pub found: bool,
    pub level: &'static str,
    pub hops: Vec<Hop>,
    /// Tree mode: (depth, symbol, call-site).
    pub tree: Vec<(usize, SymRef, String)>,
    pub note: Option<String>,
}

pub struct TraceOptions {
    pub direction: Direction,
    pub depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Callees,
    Callers,
}

impl Index {
    fn symbols_of(&self, t: Target) -> Vec<u32> {
        match t {
            Target::Symbol(s) => vec![s],
            Target::File(f) => self.file_symbols(f).map(|s| s as u32).collect(),
        }
    }

    fn file_of(&self, t: Target) -> u32 {
        match t {
            Target::File(f) => f,
            Target::Symbol(s) => self.symbols[s as usize].file,
        }
    }

    pub fn trace(&self, from: &str, to: Option<&str>, opts: &TraceOptions) -> Result<TraceResult, String> {
        let (ft, _) = self.resolve(from)?;
        let from_label = self.target_label(ft);
        let Some(to) = to else {
            return Ok(self.trace_tree(ft, from_label, opts));
        };
        let (tt, _) = self.resolve(to)?;
        let to_label = self.target_label(tt);
        let sources = self.symbols_of(ft);
        let targets: HashSet<u32> = self.symbols_of(tt).into_iter().collect();
        if let Some(path) = self.bfs_sym(&sources, &targets, false) {
            return Ok(TraceResult { from: from_label, to: Some(to_label), found: true, level: "calls", hops: path, tree: vec![], note: None });
        }
        if let Some(path) = self.bfs_sym(&self.symbols_of(tt), &sources.iter().copied().collect(), false) {
            return Ok(TraceResult {
                from: from_label,
                to: Some(to_label),
                found: true,
                level: "calls",
                hops: path,
                tree: vec![],
                note: Some("No path in that direction; this is the reverse path (the target reaches the source).".into()),
            });
        }
        let (a, b) = (self.file_of(ft), self.file_of(tt));
        if let Some(path) = self.bfs_file(a, b) {
            return Ok(TraceResult {
                from: from_label,
                to: Some(to_label),
                found: true,
                level: "files",
                hops: path,
                tree: vec![],
                note: Some("No call path; this is the file dependency path.".into()),
            });
        }
        Ok(TraceResult {
            from: from_label,
            to: Some(to_label),
            found: false,
            level: "none",
            hops: vec![],
            tree: vec![],
            note: Some("No call or dependency path found (dynamic dispatch, reflection or config wiring may connect them).".into()),
        })
    }

    fn bfs_sym(&self, sources: &[u32], targets: &HashSet<u32>, _undirected: bool) -> Option<Vec<Hop>> {
        let mut prev: HashMap<u32, (u32, u32)> = HashMap::new();
        let mut q: VecDeque<u32> = VecDeque::new();
        let mut seen: HashSet<u32> = HashSet::new();
        for &s in sources {
            if targets.contains(&s) && sources.len() == 1 {
                continue;
            }
            seen.insert(s);
            q.push_back(s);
        }
        while let Some(u) = q.pop_front() {
            if targets.contains(&u) && !sources.contains(&u) {
                let mut hops = Vec::new();
                let mut cur = u;
                while let Some(&(p, e)) = prev.get(&cur) {
                    let edge = &self.sym_edges[e as usize];
                    let caller = &self.symbols[p as usize];
                    hops.push(Hop {
                        from: self.target_label(Target::Symbol(p)),
                        to: self.target_label(Target::Symbol(cur)),
                        via: if edge.kind == EdgeKind::Extends { "extends" } else { "calls" },
                        at: format!("{}:{}", self.files[caller.file as usize].path, edge.line),
                    });
                    cur = p;
                }
                hops.reverse();
                return Some(hops);
            }
            if seen.len() > 200_000 {
                break;
            }
            for &e in &self.sym_out[u as usize] {
                let v = self.sym_edges[e as usize].to;
                if seen.insert(v) {
                    prev.insert(v, (u, e));
                    q.push_back(v);
                }
            }
        }
        None
    }

    fn bfs_file(&self, a: u32, b: u32) -> Option<Vec<Hop>> {
        if a == b {
            return None;
        }
        let mut prev: HashMap<u32, u32> = HashMap::new();
        let mut q = VecDeque::from([a]);
        let mut seen = HashSet::from([a]);
        while let Some(u) = q.pop_front() {
            if u == b {
                let mut hops = Vec::new();
                let mut cur = b;
                while let Some(&p) = prev.get(&cur) {
                    let e = self.file_out[p as usize].iter().map(|&e| &self.file_edges[e as usize]).find(|e| e.to == cur);
                    let via = match e {
                        Some(e) if e.imports > 0 => "imports",
                        _ => "calls into",
                    };
                    hops.push(Hop {
                        from: self.files[p as usize].path.clone(),
                        to: self.files[cur as usize].path.clone(),
                        via,
                        at: self.files[p as usize].path.clone(),
                    });
                    cur = p;
                }
                hops.reverse();
                return Some(hops);
            }
            for &e in &self.file_out[u as usize] {
                let v = self.file_edges[e as usize].to;
                if seen.insert(v) {
                    prev.insert(v, u);
                    q.push_back(v);
                }
            }
        }
        None
    }

    fn trace_tree(&self, t: Target, label: String, opts: &TraceOptions) -> TraceResult {
        let mut tree = Vec::new();
        let mut seen: HashSet<u32> = HashSet::new();
        let mut q: VecDeque<(u32, usize)> = VecDeque::new();
        for s in self.symbols_of(t) {
            seen.insert(s);
            q.push_back((s, 0));
        }
        let depth = opts.depth.clamp(1, 6);
        while let Some((u, d)) = q.pop_front() {
            if d >= depth || tree.len() >= 80 {
                continue;
            }
            let edges = match opts.direction {
                Direction::Callees => &self.sym_out[u as usize],
                Direction::Callers => &self.sym_in[u as usize],
            };
            for &e in edges.iter().take(12) {
                let edge = &self.sym_edges[e as usize];
                let v = if opts.direction == Direction::Callees { edge.to } else { edge.from };
                if !seen.insert(v) {
                    continue;
                }
                let site_file = self.symbols[edge.from as usize].file;
                tree.push((d + 1, self.sym_ref(v), format!("{}:{}", self.files[site_file as usize].path, edge.line)));
                q.push_back((v, d + 1));
            }
        }
        TraceResult {
            from: label,
            to: None,
            found: !tree.is_empty(),
            level: if opts.direction == Direction::Callees { "callees" } else { "callers" },
            hops: vec![],
            tree,
            note: None,
        }
    }
}

impl TraceResult {
    pub fn text(&self) -> String {
        let mut o = String::new();
        match &self.to {
            Some(to) => {
                let _ = writeln!(o, "# Trace: {} → {}", self.from, to);
                if let Some(n) = &self.note {
                    let _ = writeln!(o, "{n}");
                }
                for (i, h) in self.hops.iter().enumerate() {
                    if i == 0 {
                        let _ = writeln!(o, "{}", h.from);
                    }
                    let _ = writeln!(o, "  └─ {} ({})\n{}", h.via, h.at, h.to);
                }
            }
            None => {
                let _ = writeln!(o, "# {}: {}", if self.level == "callees" { "Calls from" } else { "Callers of" }, self.from);
                if self.tree.is_empty() {
                    let _ = writeln!(o, "(none resolved)");
                }
                for (d, s, at) in &self.tree {
                    let _ = writeln!(o, "{}{} {} — {}:{} (via {})", "  ".repeat(*d), s.kind, s.name, s.file, s.line, at);
                }
            }
        }
        o
    }
}

// ---------------------------------------------------------------- impact

#[derive(Debug, Serialize)]
pub struct ImpactResult {
    pub targets: Vec<String>,
    pub risk: &'static str,
    pub summary: String,
    /// depth -> affected symbols with the call site that links them.
    pub by_depth: Vec<Vec<Site>>,
    pub files: Vec<String>,
    pub modules: Vec<String>,
    pub tests: Vec<String>,
    pub importers: Vec<String>,
}

pub struct ImpactOptions {
    pub depth: usize,
    pub limit: usize,
}

impl Default for ImpactOptions {
    fn default() -> Self {
        Self { depth: 3, limit: 30 }
    }
}

impl Index {
    pub fn impact(&self, targets: &[String], opts: &ImpactOptions) -> Result<ImpactResult, String> {
        let mut ts = Vec::new();
        for t in targets {
            ts.push(self.resolve(t)?.0);
        }
        Ok(self.impact_of(&ts, opts))
    }

    /// Impact of the working-tree diff against `base` (default HEAD).
    pub fn impact_changed(&self, base: Option<&str>, opts: &ImpactOptions) -> Result<ImpactResult, String> {
        let base = base.unwrap_or("HEAD");
        let diff = crate::index::git_run(&self.root, &["diff", "--unified=0", "--no-color", "--no-ext-diff", base])
            .unwrap_or_default();
        let mut targets: Vec<Target> = Vec::new();
        let mut cur: Option<u32> = None;
        for line in diff.lines() {
            if let Some(p) = line.strip_prefix("+++ ") {
                cur = p.strip_prefix("b/").and_then(|p| self.path_ix.get(p).copied());
                continue;
            }
            if let (Some(f), Some(h)) = (cur, line.strip_prefix("@@ ")) {
                // @@ -a,b +c,d @@
                let plus = h.split_whitespace().find(|t| t.starts_with('+')).unwrap_or("+0");
                let mut it = plus[1..].split(',');
                let start: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                let count: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(1);
                let end = start + count.max(1) - 1;
                let hit: Vec<u32> = self
                    .file_symbols(f)
                    .filter(|&s| {
                        let sym = &self.symbols[s];
                        sym.start <= end.max(start) && start.max(1) <= sym.end && !sym.kind.is_container()
                    })
                    .map(|s| s as u32)
                    .collect();
                if hit.is_empty() {
                    let t = Target::File(f);
                    if !targets.contains(&t) {
                        targets.push(t);
                    }
                }
                for s in hit {
                    let t = Target::Symbol(s);
                    if !targets.contains(&t) {
                        targets.push(t);
                    }
                }
            }
        }
        if let Some(untracked) = crate::index::git_run(&self.root, &["ls-files", "--others", "--exclude-standard"]) {
            for p in untracked.lines() {
                if let Some(&f) = self.path_ix.get(p) {
                    targets.push(Target::File(f));
                }
            }
        }
        if targets.is_empty() {
            return Err(format!("no changed code against {base} (git diff is empty or outside indexed files)"));
        }
        Ok(self.impact_of(&targets, opts))
    }

    fn impact_of(&self, targets: &[Target], opts: &ImpactOptions) -> ImpactResult {
        let mut seen: HashSet<u32> = HashSet::new();
        let mut frontier: Vec<u32> = Vec::new();
        let mut target_files: HashSet<u32> = HashSet::new();
        for &t in targets {
            target_files.insert(self.file_of(t));
            for s in self.symbols_of(t) {
                if seen.insert(s) {
                    frontier.push(s);
                }
            }
        }
        let depth = opts.depth.clamp(1, 6);
        let mut by_depth: Vec<Vec<Site>> = Vec::new();
        let mut affected_files: HashSet<u32> = HashSet::new();
        let mut total = 0usize;
        for _ in 0..depth {
            let mut next = Vec::new();
            let mut level = Vec::new();
            for &u in &frontier {
                for &e in &self.sym_in[u as usize] {
                    let edge = &self.sym_edges[e as usize];
                    if seen.insert(edge.from) {
                        next.push(edge.from);
                        affected_files.insert(self.symbols[edge.from as usize].file);
                        level.push(Site {
                            symbol: self.sym_ref(edge.from),
                            at: edge.line,
                            via: if edge.kind == EdgeKind::Extends { "extends" } else { "calls" },
                        });
                    }
                }
            }
            total += level.len();
            level.sort_by(|a, b| (a.symbol.file.as_str(), a.at).cmp(&(b.symbol.file.as_str(), b.at)));
            if level.is_empty() {
                break;
            }
            by_depth.push(level);
            frontier = next;
        }
        // File-level importers of target files (catches re-exports and
        // module-level use that call resolution cannot see).
        let mut importers: Vec<String> = Vec::new();
        for &f in &target_files {
            for &e in &self.file_in[f as usize] {
                let from = self.file_edges[e as usize].from;
                if !target_files.contains(&from) {
                    affected_files.insert(from);
                    let p = self.files[from as usize].path.clone();
                    if !importers.contains(&p) {
                        importers.push(p);
                    }
                }
            }
        }
        importers.sort();
        let mut tests: Vec<String> = affected_files
            .iter()
            .chain(target_files.iter())
            .filter(|f| self.files[**f as usize].is_test)
            .map(|f| self.files[*f as usize].path.clone())
            .collect();
        tests.sort();
        tests.dedup();
        let mut modules: Vec<String> = affected_files
            .iter()
            .chain(target_files.iter())
            .map(|f| self.community_name(*f).to_string())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        modules.sort();
        let direct = by_depth.first().map_or(0, |v| v.len());
        let max_rank = targets
            .iter()
            .flat_map(|t| self.symbols_of(*t))
            .map(|s| self.sym_rank[s as usize])
            .fold(0f32, f32::max);
        let hot = {
            let mut r: Vec<f32> = self.sym_rank.clone();
            r.sort_by(|a, b| b.partial_cmp(a).unwrap());
            let cut = r.get(r.len() / 50).copied().unwrap_or(f32::MAX);
            max_rank >= cut && max_rank > 0.0
        };
        let risk = if direct >= 20 || total >= 60 || modules.len() >= 6 || (hot && direct >= 5) {
            "high"
        } else if direct >= 5 || total >= 15 || modules.len() >= 3 || importers.len() >= 10 {
            "medium"
        } else {
            "low"
        };
        let mut files: Vec<String> = affected_files.iter().map(|f| self.files[*f as usize].path.clone()).collect();
        files.sort();
        let summary = format!(
            "{}, {} affected in total across {} and {}; {} importing the changed files; {}.",
            plural(direct, "direct caller"),
            plural(total, "symbol"),
            plural(files.len(), "file"),
            plural(modules.len(), "module"),
            plural(importers.len(), "file"),
            match tests.len() {
                0 => "no tests reach this".to_string(),
                n => format!("{} to run", plural(n, "test file")),
            }
        );
        for level in by_depth.iter_mut() {
            level.truncate(opts.limit);
        }
        importers.truncate(opts.limit);
        ImpactResult {
            targets: targets.iter().map(|t| self.target_label(*t)).take(30).collect(),
            risk,
            summary,
            by_depth,
            files,
            modules,
            tests,
            importers,
        }
    }
}

impl ImpactResult {
    pub fn text(&self) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "# Impact ({} risk)", self.risk.to_uppercase());
        let _ = writeln!(o, "Changing: {}", self.targets.join("; "));
        let _ = writeln!(o, "{}", self.summary);
        let titles = ["Direct callers (will break if the contract changes)", "Indirect (depth 2)", "Indirect (depth 3)", "Depth 4", "Depth 5", "Depth 6"];
        for (i, level) in self.by_depth.iter().enumerate() {
            let _ = writeln!(o, "\n## {}", titles[i.min(5)]);
            for s in level {
                let _ = writeln!(o, "- {} {} — {}:{}", s.symbol.kind, s.symbol.name, s.symbol.file, s.at);
            }
        }
        if !self.importers.is_empty() {
            let _ = writeln!(o, "\n## Importing files");
            for p in &self.importers {
                let _ = writeln!(o, "- {p}");
            }
        }
        if !self.modules.is_empty() {
            let _ = writeln!(o, "\n## Modules touched\n{}", self.modules.join(", "));
        }
        if !self.tests.is_empty() {
            let _ = writeln!(o, "\n## Tests to run");
            for t in &self.tests {
                let _ = writeln!(o, "- {t}");
            }
        }
        o
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n).collect();
        t.push('…');
        t
    }
}

fn number(code: &str, start: u32) -> String {
    code.lines()
        .enumerate()
        .map(|(i, l)| format!("{:>5}| {}", start + i as u32, l))
        .collect::<Vec<_>>()
        .join("\n")
}
