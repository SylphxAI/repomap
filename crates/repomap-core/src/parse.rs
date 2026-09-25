//! Per-file extraction: tree-sitter definitions, calls, imports and heritage
//! references, plus BM25 chunks. The output is cached per file.

use crate::lang::Lang;
use crate::tokenize::chunk_terms;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, QueryCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Type,
    Module,
    Macro,
}

impl Kind {
    pub fn from_capture(s: &str) -> Option<Kind> {
        Some(match s {
            "function" => Kind::Function,
            "method" => Kind::Method,
            "class" => Kind::Class,
            "struct" => Kind::Struct,
            "interface" => Kind::Interface,
            "trait" => Kind::Trait,
            "enum" => Kind::Enum,
            "type" => Kind::Type,
            "module" => Kind::Module,
            "macro" => Kind::Macro,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Function => "function",
            Kind::Method => "method",
            Kind::Class => "class",
            Kind::Struct => "struct",
            Kind::Interface => "interface",
            Kind::Trait => "trait",
            Kind::Enum => "enum",
            Kind::Type => "type",
            Kind::Module => "module",
            Kind::Macro => "macro",
        }
    }
    pub fn is_container(self) -> bool {
        matches!(
            self,
            Kind::Class | Kind::Struct | Kind::Interface | Kind::Trait | Kind::Enum | Kind::Module
        )
    }
    pub fn is_callable(self) -> bool {
        matches!(self, Kind::Function | Kind::Method | Kind::Macro)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolFact {
    pub name: String,
    pub kind: Kind,
    /// 1-based inclusive line range of the whole definition.
    pub start: u32,
    pub end: u32,
    /// Owning type or container name (class, impl type, Go receiver).
    pub owner: Option<String>,
    /// Index of the enclosing symbol in the same file.
    pub parent: Option<u32>,
    /// First line of the definition, trimmed.
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallFact {
    pub name: String,
    pub line: u32,
    /// Enclosing symbol index in the same file (None = top level).
    pub from: Option<u32>,
    /// true for heritage references (extends / implements).
    pub heritage: bool,
    /// Receiver or qualifier for member / scoped calls (`this`, `Foo`, `pkg`).
    pub qual: Option<String>,
    /// true for `obj.m()` / `Type::f()` style calls.
    pub member: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub start: u32,
    pub end: u32,
    pub symbol: Option<u32>,
    /// (term, frequency), terms sorted.
    /// Distinct terms, space-separated and sorted (one allocation per chunk).
    pub terms: String,
    /// Frequency of each term in `terms`, same order.
    pub tfs: Vec<u16>,
    pub len: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileFacts {
    pub lang: Option<Lang>,
    pub lines: u32,
    pub symbols: Vec<SymbolFact>,
    pub calls: Vec<CallFact>,
    pub imports: Vec<String>,
    pub chunks: Vec<Chunk>,
}

thread_local! {
    static PARSERS: RefCell<HashMap<Lang, Parser>> = RefCell::new(HashMap::new());
}

const MAX_CHUNK_LINES: u32 = 150;
const WINDOW: u32 = 60;

pub fn extract(path: &str, lang: Option<Lang>, src: &str) -> FileFacts {
    let line_starts = line_starts(src);
    let lines = line_starts.len() as u32;
    let mut facts = FileFacts {
        lang,
        lines,
        ..Default::default()
    };
    if let Some(lang) = lang {
        parse_code(lang, src, &line_starts, &mut facts);
    }
    facts.chunks = build_chunks(path, src, &line_starts, &facts.symbols);
    facts
}

fn line_starts(src: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' && i + 1 < src.len() {
            starts.push(i + 1);
        }
    }
    starts
}

fn line_of(line_starts: &[usize], byte: usize) -> u32 {
    match line_starts.binary_search(&byte) {
        Ok(i) => i as u32 + 1,
        Err(i) => i as u32,
    }
}

struct RawDef {
    kind: Kind,
    name: String,
    start_byte: usize,
    end_byte: usize,
    owner: Option<String>,
}

fn parse_code(lang: Lang, src: &str, line_starts: &[usize], facts: &mut FileFacts) {
    let tree = PARSERS.with(|cell| {
        let mut map = cell.borrow_mut();
        let parser = map.entry(lang).or_insert_with(|| {
            let mut p = Parser::new();
            p.set_language(&lang.grammar()).expect("grammar version");
            p
        });
        parser.parse(src, None)
    });
    let Some(tree) = tree else { return };
    let query = lang.query();
    let names = query.capture_names();
    let bytes = src.as_bytes();
    let text = |n: tree_sitter::Node| n.utf8_text(bytes).unwrap_or("").to_string();

    let mut defs: Vec<RawDef> = Vec::new();
    let mut impls: Vec<(usize, usize, String)> = Vec::new();
    let mut calls: Vec<(String, usize, bool, Option<String>, bool)> = Vec::new();
    let mut imports: Vec<String> = Vec::new();

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), bytes);
    while let Some(m) = matches.next() {
        let mut def: Option<(Kind, tree_sitter::Node)> = None;
        let mut name: Option<String> = None;
        let mut owner: Option<String> = None;
        let mut scope_impl: Option<tree_sitter::Node> = None;
        let mut scope_type: Option<String> = None;
        let mut import_from: Option<String> = None;
        let mut import_name: Option<String> = None;
        let mut qual: Option<String> = None;
        let mut mcall: Option<(String, usize)> = None;
        for cap in m.captures() {
            let cname = names[cap.index as usize];
            match cname {
                "name" => name = Some(text(cap.node)),
                "owner" => owner = Some(text(cap.node)),
                "call" => calls.push((text(cap.node), cap.node.start_byte(), false, None, false)),
                "mcall" => mcall = Some((text(cap.node), cap.node.start_byte())),
                "qual" => qual = Some(qualifier(&text(cap.node))),
                "ref" => calls.push((text(cap.node), cap.node.start_byte(), true, None, false)),
                "import" => imports.push(clean_import(&text(cap.node))),
                "import.mod" => imports.push(format!("mod:{}", text(cap.node))),
                "import.from" => import_from = Some(text(cap.node)),
                "import.name" => import_name = Some(text(cap.node)),
                "scope.impl" => scope_impl = Some(cap.node),
                "scope.type" => scope_type = Some(text(cap.node)),
                other => {
                    if let Some(kind) = other.strip_prefix("def.").and_then(Kind::from_capture) {
                        def = Some((kind, cap.node));
                    }
                }
            }
        }
        if let Some((name, byte)) = mcall {
            calls.push((name, byte, false, qual.take(), true));
        }
        if let (Some(from), Some(n)) = (import_from, import_name) {
            let joined = if from.ends_with('.') {
                format!("{from}{n}")
            } else {
                format!("{from}.{n}")
            };
            imports.push(joined);
        }
        if let (Some(node), Some(ty)) = (scope_impl, scope_type) {
            impls.push((node.start_byte(), node.end_byte(), ty));
        }
        if let (Some((kind, node)), Some(name)) = (def, name) {
            if name.is_empty() || name.len() > 120 {
                continue;
            }
            defs.push(RawDef {
                kind,
                name,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                owner: owner.map(|o| o.rsplit("::").next().unwrap_or(&o).to_string()),
            });
        }
    }

    // Deduplicate: the same node may match several patterns (Go type specs).
    defs.sort_by(|a, b| {
        a.start_byte
            .cmp(&b.start_byte)
            .then(b.end_byte.cmp(&a.end_byte))
    });
    defs.dedup_by(|b, a| a.start_byte == b.start_byte && a.name == b.name);

    // Nesting via a stack of open definitions.
    let mut stack: Vec<usize> = Vec::new();
    let mut symbols: Vec<SymbolFact> = Vec::with_capacity(defs.len());
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(defs.len());
    for d in &defs {
        while let Some(&top) = stack.last() {
            if spans[top].1 <= d.start_byte {
                stack.pop();
            } else {
                break;
            }
        }
        let parent = stack.last().copied();
        let mut kind = d.kind;
        let mut owner = d.owner.clone();
        if owner.is_none() {
            if let Some(p) = parent {
                if symbols[p].kind.is_container() {
                    owner = Some(symbols[p].name.clone());
                }
            }
        }
        if owner.is_none() {
            if let Some((_, _, ty)) = impls
                .iter()
                .filter(|(s, e, _)| *s <= d.start_byte && d.end_byte <= *e)
                .min_by_key(|(s, e, _)| e - s)
            {
                owner = Some(ty.clone());
            }
        }
        if kind == Kind::Function && owner.is_some() && parent.map_or(true, |p| symbols[p].kind.is_container()) {
            kind = Kind::Method;
        }
        let start = line_of(line_starts, d.start_byte);
        let end = line_of(line_starts, d.end_byte.saturating_sub(1).max(d.start_byte));
        let sig_line = &src[line_starts[(start - 1) as usize]..];
        let sig_line = sig_line.lines().next().unwrap_or("").trim();
        let signature: String = sig_line.chars().take(160).collect();
        symbols.push(SymbolFact {
            name: d.name.clone(),
            kind,
            start,
            end,
            owner,
            parent: parent.map(|p| p as u32),
            signature,
        });
        spans.push((d.start_byte, d.end_byte));
        stack.push(symbols.len() - 1);
    }

    // Attribute each call to its innermost enclosing callable/container definition.
    let mut call_facts = Vec::with_capacity(calls.len());
    for (name, byte, heritage, qual, member) in calls {
        if name.is_empty() || name.len() > 120 {
            continue;
        }
        let from = innermost(&spans, byte);
        call_facts.push(CallFact {
            name,
            line: line_of(line_starts, byte),
            from: from.map(|i| i as u32),
            heritage,
            qual,
            member,
        });
    }
    imports.retain(|s| !s.is_empty());
    imports.sort();
    imports.dedup();
    facts.symbols = symbols;
    facts.calls = call_facts;
    facts.imports = imports;
}

fn innermost(spans: &[(usize, usize)], byte: usize) -> Option<usize> {
    // spans are sorted by start; the innermost containing span is the last one
    // starting before `byte` that also ends after it.
    let idx = spans.partition_point(|(s, _)| *s <= byte);
    let mut best: Option<usize> = None;
    for i in (0..idx).rev() {
        let (s, e) = spans[i];
        if s <= byte && byte < e {
            match best {
                Some(b) if spans[b].1 - spans[b].0 <= e - s => {}
                _ => best = Some(i),
            }
            // A containing span found; ancestors are wider, so stop once we
            // reach a span that cannot be tighter.
            if best == Some(i) {
                break;
            }
        }
    }
    best
}

/// Last path segment of a receiver expression: `a.b.c` -> `c`, `crate::x::Foo` -> `Foo`,
/// `this` -> `this`. Complex receivers (calls, literals) yield an empty string.
fn qualifier(raw: &str) -> String {
    let t = raw.trim();
    if t.len() > 80 || t.contains(['(', '[', '"', '\'', ' ', '\n']) {
        return String::new();
    }
    let t = t.trim_start_matches('&').trim_start_matches('*').trim_start_matches('$').trim_start_matches('@');
    let last = t.rsplit(|c| c == '.' || c == ':' || c == '>' || c == '\\').find(|s| !s.is_empty()).unwrap_or(t);
    last.split('<').next().unwrap_or(last).to_string()
}

fn clean_import(raw: &str) -> String {
    let t = raw.trim();
    let t = t.trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '<' || c == '>');
    t.to_string()
}

fn build_chunks(path: &str, src: &str, line_starts: &[usize], symbols: &[SymbolFact]) -> Vec<Chunk> {
    let lines = line_starts.len() as u32;
    let mut out: Vec<(u32, u32, Option<u32>)> = Vec::new();
    let mut covered = vec![false; lines as usize + 2];

    // Top-down: a definition that fits becomes one chunk; a large one yields a
    // header chunk and lets its children chunk themselves.
    let mut children: Vec<Vec<u32>> = vec![Vec::new(); symbols.len()];
    let mut roots = Vec::new();
    for (i, s) in symbols.iter().enumerate() {
        match s.parent {
            Some(p) => children[p as usize].push(i as u32),
            None => roots.push(i as u32),
        }
    }
    let mut work = roots;
    while let Some(i) = work.pop() {
        let s = &symbols[i as usize];
        let span = s.end.saturating_sub(s.start) + 1;
        if span <= MAX_CHUNK_LINES || children[i as usize].is_empty() {
            let end = s.end.min(s.start + MAX_CHUNK_LINES * 2);
            out.push((s.start, end, Some(i)));
            for l in s.start..=end {
                covered[l as usize] = true;
            }
        } else {
            let first_child = children[i as usize]
                .iter()
                .map(|c| symbols[*c as usize].start)
                .min()
                .unwrap_or(s.end);
            let header_end = (first_child.saturating_sub(1)).max(s.start).min(s.start + 40);
            out.push((s.start, header_end, Some(i)));
            for l in s.start..=header_end {
                covered[l as usize] = true;
            }
            work.extend(children[i as usize].iter().copied());
        }
    }

    // Uncovered regions become fixed windows.
    let mut l = 1;
    while l <= lines {
        if covered[l as usize] {
            l += 1;
            continue;
        }
        let start = l;
        let mut end = l;
        while end < lines && !covered[end as usize + 1] && end - start + 1 < WINDOW {
            end += 1;
        }
        out.push((start, end, None));
        l = end + 1;
    }

    let path_terms = crate::tokenize::path_terms(path);
    let mut chunks = Vec::with_capacity(out.len());
    for (start, end, sym) in out {
        let a = line_starts[(start - 1) as usize];
        let b = if (end as usize) < line_starts.len() {
            line_starts[end as usize]
        } else {
            src.len()
        };
        let body = &src[a..b];
        if body.trim().is_empty() {
            continue;
        }
        let name = sym.map(|i| symbols[i as usize].name.as_str());
        let (terms, len) = chunk_terms(body, name, &path_terms);
        if terms.is_empty() {
            continue;
        }
        let mut joined = String::with_capacity(terms.iter().map(|(t, _)| t.len() + 1).sum());
        let mut tfs = Vec::with_capacity(terms.len());
        for (t, f) in terms {
            if !joined.is_empty() {
                joined.push(' ');
            }
            joined.push_str(&t);
            tfs.push(f);
        }
        chunks.push(Chunk {
            start,
            end,
            symbol: sym,
            terms: joined,
            tfs,
            len,
        });
    }
    chunks.sort_by_key(|c| c.start);
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(f: &FileFacts) -> Vec<String> {
        f.symbols
            .iter()
            .map(|s| match &s.owner {
                Some(o) => format!("{}:{}.{}", s.kind.as_str(), o, s.name),
                None => format!("{}:{}", s.kind.as_str(), s.name),
            })
            .collect()
    }

    #[test]
    fn typescript() {
        let src = r#"import { a } from './a';
export class Foo extends Base implements Bar {
  run(x: number) { return helper(x); }
  private go = () => this.run(1);
}
export function helper(n: number) { return new Foo(); }
export const arrow = (y) => helper(y);
interface Bar { run(x: number): number }
type Alias = string;
"#;
        let f = extract("src/x.ts", Some(Lang::TypeScript), src);
        let n = names(&f);
        assert!(n.contains(&"class:Foo".into()), "{n:?}");
        assert!(n.contains(&"method:Foo.run".into()), "{n:?}");
        assert!(n.contains(&"method:Foo.go".into()), "{n:?}");
        assert!(n.contains(&"function:helper".into()), "{n:?}");
        assert!(n.contains(&"function:arrow".into()), "{n:?}");
        assert!(n.contains(&"interface:Bar".into()), "{n:?}");
        assert!(n.contains(&"type:Alias".into()), "{n:?}");
        assert_eq!(f.imports, vec!["./a".to_string()]);
        let run = f.symbols.iter().position(|s| s.name == "run").unwrap() as u32;
        assert!(f.calls.iter().any(|c| c.name == "helper" && c.from == Some(run)));
        assert!(f.calls.iter().any(|c| c.name == "Base" && c.heritage));
        assert!(!f.chunks.is_empty());
    }

    #[test]
    fn python() {
        let src = "from .models import User\nimport os.path\n\nclass Service(Base):\n    def get(self, id):\n        return fetch(id)\n\ndef fetch(id):\n    return User(id)\n";
        let f = extract("app/service.py", Some(Lang::Python), src);
        let n = names(&f);
        assert!(n.contains(&"class:Service".into()), "{n:?}");
        assert!(n.contains(&"method:Service.get".into()), "{n:?}");
        assert!(n.contains(&"function:fetch".into()), "{n:?}");
        assert!(f.imports.contains(&".models".to_string()), "{:?}", f.imports);
        assert!(f.imports.contains(&".models.User".to_string()), "{:?}", f.imports);
        assert!(f.imports.contains(&"os.path".to_string()));
    }

    #[test]
    fn rust_impl_owner() {
        let src = "use crate::a::b;\nmod util;\nstruct S;\nimpl Display for S {\n    fn fmt(&self) { helper(); }\n}\nfn helper() {}\n";
        let f = extract("src/lib.rs", Some(Lang::Rust), src);
        let n = names(&f);
        assert!(n.contains(&"method:S.fmt".into()), "{n:?}");
        assert!(n.contains(&"function:helper".into()), "{n:?}");
        assert!(f.imports.contains(&"mod:util".to_string()), "{:?}", f.imports);
        assert!(f.imports.contains(&"crate::a::b".to_string()), "{:?}", f.imports);
        assert!(f.calls.iter().any(|c| c.name == "Display" && c.heritage));
    }

    #[test]
    fn go_receiver() {
        let src = "package x\nimport \"fmt\"\ntype Server struct{}\nfunc (s *Server) Start() { fmt.Println(\"hi\"); run() }\nfunc run() {}\n";
        let f = extract("x.go", Some(Lang::Go), src);
        let n = names(&f);
        assert!(n.contains(&"method:Server.Start".into()), "{n:?}");
        assert!(n.contains(&"struct:Server".into()), "{n:?}");
        assert_eq!(f.imports, vec!["fmt".to_string()]);
    }

    #[test]
    fn other_languages_extract_something() {
        let cases = [
            ("A.java", Lang::Java, "import a.b.C;\nclass A extends B { void run() { go(); } }"),
            ("a.c", Lang::C, "#include \"a.h\"\nint main(void) { run(); return 0; }"),
            ("a.cpp", Lang::Cpp, "#include <vector>\nclass A {}; void A::run() { go(); }"),
            ("a.cs", Lang::CSharp, "using System.IO;\nclass A : B { void Run() { Go(); } }"),
            ("a.rb", Lang::Ruby, "require 'x'\nclass A < B\n  def run\n    go(1)\n  end\nend\n"),
            ("a.php", Lang::Php, "<?php\nuse App\\Models\\User;\nclass A extends B { function run() { go(); } }"),
            ("a.js", Lang::JavaScript, "const x = require('./x');\nclass A extends B { run() { go(); } }\n"),
        ];
        for (path, lang, src) in cases {
            let f = extract(path, Some(lang), src);
            assert!(!f.symbols.is_empty(), "{path}: no symbols");
            assert!(!f.calls.is_empty(), "{path}: no calls");
            assert!(!f.imports.is_empty(), "{path}: no imports");
        }
    }
}
