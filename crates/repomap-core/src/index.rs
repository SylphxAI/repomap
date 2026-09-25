//! Repository walk, per-file cache, and the assembled in-memory index.

use crate::bm25::Bm25;
use crate::graph;
use crate::lang::{is_searchable_text, Lang};
use crate::parse::{self, FileFacts, Kind};
use anyhow::{Context, Result};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Instant, UNIX_EPOCH};

const CACHE_VERSION: u32 = 7;
const MAX_FILE_BYTES: u64 = 1_000_000;

/// Directories skipped even when they are not git-ignored.
const DEFAULT_IGNORES: &[&str] = &[
    "node_modules", "bower_components", "vendor", "third_party", "dist", "build", "out", "target",
    "coverage", "__pycache__", ".venv", "venv", ".tox", ".mypy_cache", ".pytest_cache", ".next",
    ".nuxt", ".svelte-kit", ".turbo", ".cache", ".gradle", ".idea", ".vscode", "Pods", "DerivedData",
];

const IGNORED_FILES: &[&str] = &[
    "*.min.js", "*.min.css", "*.map", "*.lock", "package-lock.json", "pnpm-lock.yaml", "yarn.lock",
    "bun.lockb", "*.pb.go", "*_pb2.py", "*.generated.*", "*.snap",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub lang: Option<Lang>,
    pub lines: u32,
    pub bytes: u64,
    pub is_test: bool,
    pub role: Role,
    /// Symbols of this file live at `symbols[sym_start..sym_end]`.
    pub sym_start: u32,
    pub sym_end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: Kind,
    pub file: u32,
    pub start: u32,
    pub end: u32,
    pub owner: Option<String>,
    pub parent: Option<u32>,
    pub signature: String,
}

impl Symbol {
    pub fn qualified(&self) -> String {
        match &self.owner {
            Some(o) => format!("{o}.{}", self.name),
            None => self.name.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeKind {
    Calls,
    Extends,
}

/// A resolved symbol-to-symbol reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymEdge {
    pub from: u32,
    pub to: u32,
    pub kind: EdgeKind,
    /// Call-site line in the caller's file.
    pub line: u32,
}

/// Aggregated file-to-file dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdge {
    pub from: u32,
    pub to: u32,
    pub imports: u32,
    pub calls: u32,
}

impl FileEdge {
    pub fn weight(&self) -> f32 {
        self.imports as f32 + self.calls as f32 * 0.5
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    pub id: u32,
    pub name: String,
    pub files: Vec<u32>,
    /// `core`, or the auxiliary role: `tests`, `examples`, `docs`, `benchmarks`.
    pub kind: String,
}

/// What a file is for. Only `Core` files form code modules; the rest are
/// grouped by role so they never name or skew the core clusters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Core,
    Test,
    Example,
    Doc,
    Bench,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Core => "core",
            Role::Test => "tests",
            Role::Example => "examples",
            Role::Doc => "docs",
            Role::Bench => "benchmarks",
        }
    }
}

pub fn role_of(path: &str, is_test: bool) -> Role {
    let mut orig: Vec<&str> = path.split('/').collect();
    orig.pop();
    for raw in orig {
        // Split "watchOS Example", "runtime-tests", "jvmTest" into words.
        for w in crate::tokenize::tokenize(raw) {
            match w.as_str() {
                "example" | "examples" | "sample" | "samples" | "demo" | "demos" | "playground" | "playgrounds" | "showcase" | "tutorial" | "tutorials" => return Role::Example,
                "docs" | "doc" | "documentation" | "website" => return Role::Doc,
                "bench" | "benches" | "benchmark" | "benchmarks" => return Role::Bench,
                "test" | "tests" | "__tests__" | "spec" | "specs" | "testdata" | "fixtures" | "__fixtures__" | "__mocks__" | "e2e" | "testing" => return Role::Test,
                _ => {}
            }
        }
    }
    if is_test { Role::Test } else { Role::Core }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub files_seen: usize,
    pub files_parsed: usize,
    pub files_cached: usize,
    pub walk_ms: u128,
    pub parse_ms: u128,
    pub graph_ms: u128,
    pub total_ms: u128,
}

pub struct Index {
    pub root: PathBuf,
    pub files: Vec<FileEntry>,
    pub path_ix: HashMap<String, u32>,
    pub symbols: Vec<Symbol>,
    pub sym_edges: Vec<SymEdge>,
    /// Per-symbol outgoing / incoming edge ids into `sym_edges`.
    pub sym_out: Vec<Vec<u32>>,
    pub sym_in: Vec<Vec<u32>>,
    pub file_edges: Vec<FileEdge>,
    pub file_out: Vec<Vec<u32>>,
    pub file_in: Vec<Vec<u32>>,
    /// Unresolved call names per file (for search and context hints).
    pub imports_raw: Vec<Vec<String>>,
    pub file_rank: Vec<f32>,
    pub sym_rank: Vec<f32>,
    pub community: Vec<u32>,
    pub communities: Vec<Community>,
    pub by_name: HashMap<String, Vec<u32>>,
    /// Lowercased symbol names, parallel to `symbols` (search hot path).
    pub names_lower: Vec<String>,
    pub bm25: Bm25,
    /// Chunk embeddings, parallel to `bm25.chunks` (empty without the model).
    pub dense: crate::semantic::Dense,
    pub stats: Stats,
    pub git: GitInfo,
    /// Hash of (path, mtime, size) for every candidate file at build time.
    pub fingerprint: u64,
    /// The embedding model the index was built with ("" for none).
    pub model_id: &'static str,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitInfo {
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub remote_web: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    /// Embedding model the chunk vectors came from ("" for none).
    embed: String,
    entries: HashMap<String, CacheEntry>,
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    mtime: u64,
    size: u64,
    facts: FileFacts,
}

pub fn cache_dir(root: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("REPOMAP_CACHE_DIR") {
        return PathBuf::from(dir).join(root_slug(root));
    }
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("repomap")
        .join(root_slug(root))
}

fn root_slug(root: &Path) -> String {
    let s = root.to_string_lossy();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    let name: String = root
        .file_name()
        .map(|n| n.to_string_lossy().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect())
        .unwrap_or_default();
    format!("{name}-{h:016x}")
}

pub fn is_test_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let name = p.rsplit('/').next().unwrap_or(&p);
    p.split('/').any(|s| matches!(s, "test" | "tests" | "__tests__" | "spec" | "specs" | "testdata" | "e2e"))
        // `test_x.py`, but not a library such as `lib/test_functions.bash`.
        || (name.starts_with("test_") && matches!(name.rsplit('.').next(), Some("py" | "rb" | "c" | "cc" | "cpp" | "lua" | "dart" | "php")))
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.contains("_test.")
        || name.contains("_spec.")
        || name.ends_with("test.java")
        || name.ends_with("tests.cs")
}

pub struct BuildOptions {
    pub use_cache: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self { use_cache: true }
    }
}

struct Candidate {
    path: String,
    abs: PathBuf,
    size: u64,
    mtime: u64,
    lang: Option<Lang>,
}

fn walk(root: &Path) -> Result<Vec<Candidate>> {
    let mut ov = OverrideBuilder::new(root);
    for d in DEFAULT_IGNORES {
        ov.add(&format!("!{d}/"))?;
    }
    for f in IGNORED_FILES {
        ov.add(&format!("!{f}"))?;
    }
    let overrides = ov.build()?;
    let out = Mutex::new(Vec::new());
    WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .add_custom_ignore_filename(".repomapignore")
        .overrides(overrides)
        .build_parallel()
        .run(|| {
            let out = &out;
            Box::new(move |entry| {
                let Ok(entry) = entry else { return ignore::WalkState::Continue };
                if !entry.file_type().map_or(false, |t| t.is_file()) {
                    return ignore::WalkState::Continue;
                }
                let abs = entry.path();
                let Ok(rel) = abs.strip_prefix(root) else { return ignore::WalkState::Continue };
                let rel = rel.to_string_lossy().replace('\\', "/");
                let lang = Lang::from_path(&rel);
                if lang.is_none() && !is_searchable_text(&rel) {
                    return ignore::WalkState::Continue;
                }
                let Ok(meta) = entry.metadata() else { return ignore::WalkState::Continue };
                if meta.len() > MAX_FILE_BYTES || meta.len() == 0 {
                    return ignore::WalkState::Continue;
                }
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos() as u64);
                out.lock().unwrap().push(Candidate {
                    path: rel,
                    abs: abs.to_path_buf(),
                    size: meta.len(),
                    mtime,
                    lang,
                });
                ignore::WalkState::Continue
            })
        });
    let mut v = out.into_inner().unwrap();
    v.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(v)
}

fn fingerprint_of(c: &[Candidate]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
    };
    for x in c {
        for b in x.path.bytes() {
            mix(b as u64);
        }
        mix(x.mtime);
        mix(x.size);
    }
    h
}

/// Cheap change detection: walk and stat without parsing.
pub fn fingerprint(root: &Path) -> Result<u64> {
    let root = root.canonicalize()?;
    Ok(fingerprint_of(&walk(&root)?))
}

fn looks_generated(src: &str) -> bool {
    let head = &src.as_bytes()[..src.len().min(8192)];
    if head.contains(&0) {
        return true;
    }
    let lines = src.lines().count().max(1);
    src.len() > 20_000 && src.len() / lines > 400
}

impl Index {
    pub fn build(root: &Path, opts: &BuildOptions) -> Result<Index> {
        let t0 = Instant::now();
        let root = root
            .canonicalize()
            .with_context(|| format!("cannot open {}", root.display()))?;
        // Load the embedding model while the walk runs.
        let loading = std::thread::spawn(|| {
            crate::semantic::model();
        });
        let candidates = walk(&root)?;
        let _ = loading.join();
        // Fixed for this build, even if a background download finishes meanwhile.
        let model_id = crate::semantic::model_id();
        let walk_ms = t0.elapsed().as_millis();
        let fp = fingerprint_of(&candidates);

        let cache_path = cache_dir(&root).join("facts.bin");
        let mut cache: HashMap<String, CacheEntry> = if opts.use_cache {
            std::fs::read(&cache_path)
                .ok()
                .and_then(|b| postcard::from_bytes::<CacheFile>(&b).ok())
                .filter(|c| c.version == CACHE_VERSION && c.embed == model_id)
                .map(|c| c.entries)
                .unwrap_or_default()
        } else {
            HashMap::new()
        };

        let t1 = Instant::now();
        // Move cache hits out of the map so nothing is cloned.
        let prior: Vec<Option<CacheEntry>> = candidates
            .iter()
            .map(|c| cache.remove(&c.path).filter(|hit| hit.mtime == c.mtime && hit.size == c.size))
            .collect();
        let stale = !cache.is_empty();
        let results: Vec<Option<(bool, CacheEntry)>> = candidates
            .par_iter()
            .zip(prior.into_par_iter())
            .map(|(c, hit)| {
                if let Some(hit) = hit {
                    return Some((true, hit));
                }
                let bytes = std::fs::read(&c.abs).ok()?;
                let src = String::from_utf8_lossy(&bytes);
                if looks_generated(&src) {
                    return None;
                }
                let facts = parse::extract(&c.path, c.lang, &src);
                Some((false, CacheEntry { mtime: c.mtime, size: c.size, facts }))
            })
            .collect();
        let parse_ms = t1.elapsed().as_millis();

        let mut stats = Stats { files_seen: candidates.len(), walk_ms, parse_ms, ..Default::default() };
        let mut kept: Vec<(&Candidate, CacheEntry)> = Vec::with_capacity(candidates.len());
        let mut dirty = false;
        for (c, r) in candidates.iter().zip(results) {
            if let Some((hit, entry)) = r {
                if hit {
                    stats.files_cached += 1;
                } else {
                    stats.files_parsed += 1;
                    dirty = true;
                }
                kept.push((c, entry));
            }
        }
        if stale {
            dirty = true;
        }
        drop(cache);

        let t2 = Instant::now();
        let mut index = assemble(root.clone(), &kept);
        index.model_id = model_id;
        stats.graph_ms = t2.elapsed().as_millis();

        if opts.use_cache && dirty {
            let entries: HashMap<String, CacheEntry> = kept
                .into_iter()
                .map(|(c, e)| (c.path.clone(), e))
                .collect();
            let file = CacheFile { version: CACHE_VERSION, embed: model_id.to_string(), entries };
            if let Ok(bytes) = postcard::to_stdvec(&file) {
                let _ = std::fs::create_dir_all(cache_path.parent().unwrap());
                let tmp = cache_path.with_extension("tmp");
                if std::fs::write(&tmp, bytes).is_ok() {
                    let _ = std::fs::rename(&tmp, &cache_path);
                }
            }
        }
        stats.total_ms = t0.elapsed().as_millis();
        index.stats = stats;
        index.git = git_info(&root);
        index.fingerprint = fp;
        Ok(index)
    }

    pub fn file_symbols(&self, file: u32) -> std::ops::Range<usize> {
        let f = &self.files[file as usize];
        f.sym_start as usize..f.sym_end as usize
    }

    pub fn code_files(&self) -> impl Iterator<Item = (u32, &FileEntry)> {
        self.files.iter().enumerate().filter(|(_, f)| f.lang.is_some()).map(|(i, f)| (i as u32, f))
    }

    /// Innermost symbol in `file` containing `line`.
    pub fn symbol_at(&self, file: u32, line: u32) -> Option<u32> {
        self.file_symbols(file)
            .filter(|&i| self.symbols[i].start <= line && line <= self.symbols[i].end)
            .min_by_key(|&i| self.symbols[i].end - self.symbols[i].start)
            .map(|i| i as u32)
    }

    pub fn read_lines(&self, file: u32, start: u32, end: u32) -> Option<String> {
        let abs = self.root.join(&self.files[file as usize].path);
        let src = std::fs::read_to_string(abs).ok()?;
        let lines: Vec<&str> = src.lines().collect();
        let s = (start.max(1) - 1) as usize;
        let e = (end as usize).min(lines.len());
        if s >= e {
            return Some(String::new());
        }
        Some(lines[s..e].join("\n"))
    }
}

fn assemble(root: PathBuf, kept: &[(&Candidate, CacheEntry)]) -> Index {
    let mut files = Vec::with_capacity(kept.len());
    let mut symbols = Vec::new();
    let mut path_ix = HashMap::with_capacity(kept.len());
    let mut imports_raw = Vec::with_capacity(kept.len());
    for (i, (c, e)) in kept.iter().enumerate() {
        let base = symbols.len() as u32;
        for s in &e.facts.symbols {
            symbols.push(Symbol {
                name: s.name.clone(),
                kind: s.kind,
                file: i as u32,
                start: s.start,
                end: s.end,
                owner: s.owner.clone(),
                parent: s.parent.map(|p| p + base),
                signature: s.signature.clone(),
            });
        }
        files.push(FileEntry {
            path: c.path.clone(),
            lang: c.lang,
            lines: e.facts.lines,
            bytes: c.size,
            is_test: is_test_path(&c.path),
            role: role_of(&c.path, is_test_path(&c.path)),
            sym_start: base,
            sym_end: symbols.len() as u32,
        });
        path_ix.insert(c.path.clone(), i as u32);
        imports_raw.push(e.facts.imports.clone());
    }
    let mut by_name: HashMap<String, Vec<u32>> = HashMap::new();
    for (i, s) in symbols.iter().enumerate() {
        by_name.entry(s.name.clone()).or_default().push(i as u32);
    }
    let names_lower: Vec<String> = symbols.iter().map(|s| s.name.to_ascii_lowercase()).collect();
    let tb = Instant::now();
    let bm25 = Bm25::build(kept.iter().enumerate().map(|(i, (_, e))| (i as u32, &files[i], &e.facts)));
    let mut dense = crate::semantic::Dense::default();
    if let Some(m) = crate::semantic::model().filter(|_| kept.iter().any(|(_, e)| e.facts.chunks.iter().any(|c| c.vec.is_some()))) {
        dense = crate::semantic::Dense::new(m.dims());
        for (_, e) in kept {
            for c in &e.facts.chunks {
                dense.push(c.vec.as_ref());
            }
        }
    }
    if std::env::var_os("REPOMAP_TRACE").is_some() {
        eprintln!("[repomap] bm25: {} ms", tb.elapsed().as_millis());
    }
    let mut index = Index {
        root,
        files,
        path_ix,
        symbols,
        sym_edges: Vec::new(),
        sym_out: Vec::new(),
        sym_in: Vec::new(),
        file_edges: Vec::new(),
        file_out: Vec::new(),
        file_in: Vec::new(),
        imports_raw,
        file_rank: Vec::new(),
        sym_rank: Vec::new(),
        community: Vec::new(),
        communities: Vec::new(),
        by_name,
        names_lower,
        bm25,
        dense,
        stats: Stats::default(),
        git: GitInfo::default(),
        fingerprint: 0,
        model_id: "",
    };
    let facts: Vec<&FileFacts> = kept.iter().map(|(_, e)| &e.facts).collect();
    graph::link(&mut index, &facts);
    index
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

pub fn git_info(root: &Path) -> GitInfo {
    let commit = git(root, &["rev-parse", "HEAD"]);
    let branch = git(root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let remote_web = git(root, &["remote", "get-url", "origin"]).and_then(|u| remote_to_web(&u));
    GitInfo { commit, branch, remote_web }
}

fn remote_to_web(url: &str) -> Option<String> {
    let u = url.trim().trim_end_matches(".git");
    if let Some(rest) = u.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return Some(format!("https://{host}/{path}"));
    }
    if u.starts_with("https://") || u.starts_with("http://") {
        // Drop credentials if any.
        let (scheme, rest) = u.split_once("://")?;
        let rest = rest.rsplit_once('@').map_or(rest, |(_, r)| r);
        return Some(format!("{scheme}://{rest}"));
    }
    None
}

pub(crate) fn git_run(root: &Path, args: &[&str]) -> Option<String> {
    git(root, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paths() {
        assert!(is_test_path("src/__tests__/a.ts"));
        assert!(is_test_path("pkg/foo_test.go"));
        assert!(is_test_path("a/b.spec.ts"));
        assert!(!is_test_path("src/testing_utils.ts"));
        assert!(is_test_path("tests_x/test_app.py") && !is_test_path("lib/bats-core/test_functions.bash"));
        assert_eq!(role_of("examples/tutorial/flaskr/db.py", false), Role::Example);
        assert_eq!(role_of("src/flask/app.py", false), Role::Core);
        assert_eq!(role_of("benchmarks/jsx/a.ts", false), Role::Bench);
        assert_eq!(role_of("src/a.test.ts", true), Role::Test);
        assert_eq!(role_of("okhttp/src/jvmTest/kotlin/A.kt", false), Role::Test);
        assert_eq!(role_of("runtime-tests/node/index.ts", false), Role::Test);
        assert_eq!(role_of("src/latest/a.ts", false), Role::Core);
        assert_eq!(role_of("watchOS Example/x/A.swift", false), Role::Example);
    }

    #[test]
    fn remotes() {
        assert_eq!(remote_to_web("git@github.com:a/b.git").as_deref(), Some("https://github.com/a/b"));
        assert_eq!(remote_to_web("https://x:y@github.com/a/b.git").as_deref(), Some("https://github.com/a/b"));
    }
}
