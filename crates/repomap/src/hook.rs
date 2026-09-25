//! `repomap hook`: a Claude Code PreToolUse hook for Grep and Glob.
//!
//! Reads the hook payload on stdin and, when the pattern names symbols or
//! files the index knows, answers with `additionalContext`: where they are
//! defined, who calls them and which module they belong to. It never blocks
//! the tool call and stays silent on any error or timeout.

use repomap_core::{BuildOptions, Index, SearchOptions};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

const MAX_CHARS: usize = 1800;

pub fn run() {
    let mut raw = String::new();
    if std::io::stdin().read_to_string(&mut raw).is_err() {
        return;
    }
    let Ok(input) = serde_json::from_str::<Value>(&raw) else { return };
    if let Some(text) = context_for(&input) {
        let out = json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "additionalContext": text,
            }
        });
        println!("{out}");
    }
}

fn repo_root(start: &Path) -> Option<PathBuf> {
    let mut d = Some(start);
    while let Some(p) = d {
        if p.join(".git").exists() {
            return Some(p.to_path_buf());
        }
        d = p.parent();
    }
    None
}

/// Identifier-like words from a regex or glob, most specific first.
pub fn words(pattern: &str) -> Vec<String> {
    // Drop regex escapes (\b, \s, \w, \d, \.) before splitting.
    let mut cleaned = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            chars.next();
            cleaned.push(' ');
        } else if c.is_ascii_alphanumeric() || c == '_' {
            cleaned.push(c);
        } else {
            cleaned.push(' ');
        }
    }
    let mut out: Vec<String> = Vec::new();
    for w in cleaned.split_whitespace() {
        if w.len() >= 3 && !w.chars().all(|c| c.is_ascii_digit()) && !out.iter().any(|o| o == w) {
            out.push(w.to_string());
        }
    }
    const NOISE: &[&str] = &["function", "class", "const", "let", "var", "def", "func", "fn", "pub", "import", "from", "export", "return", "async", "await", "impl", "struct", "interface", "type", "src", "test", "tests", "spec", "tsx", "jsx", "js", "ts", "rs", "py", "go", "java", "kt", "swift", "rb", "php", "cpp", "hpp"];
    out.retain(|w| !NOISE.contains(&w.to_ascii_lowercase().as_str()));
    out.sort_by_key(|w| std::cmp::Reverse(w.len()));
    out.truncate(3);
    out
}

fn context_for(input: &Value) -> Option<String> {
    let tool = input.get("tool_name")?.as_str()?;
    let args = input.get("tool_input")?;
    let pattern = args.get("pattern")?.as_str()?;
    let cwd = input.get("cwd").and_then(|v| v.as_str()).map(PathBuf::from).or_else(|| std::env::current_dir().ok())?;
    let root = repo_root(&cwd)?;
    if Some(&root) == dirs::home_dir().as_ref() {
        return None;
    }
    let ws = words(pattern);
    if ws.is_empty() {
        return None;
    }
    let timeout = std::env::var("REPOMAP_HOOK_TIMEOUT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(Index::build(&root, &BuildOptions::default()).ok());
    });
    let index = rx.recv_timeout(Duration::from_millis(timeout)).ok()??;
    let text = match tool {
        "Grep" => grep_context(&index, &ws),
        "Glob" => glob_context(&index, &ws),
        _ => None,
    }?;
    let mut text = format!("repomap (code map) for this search:\n{text}Next: repomap `context <symbol>` for code + callers, `impact <symbol>` before editing.");
    if text.len() > MAX_CHARS {
        let mut cut = MAX_CHARS;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push('…');
    }
    Some(text)
}

fn module_of(index: &Index, file: u32) -> String {
    index
        .community
        .get(file as usize)
        .and_then(|c| index.communities.get(*c as usize))
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

fn grep_context(index: &Index, words: &[String]) -> Option<String> {
    let mut out = String::new();
    let mut shown = 0;
    for w in words {
        let Some(defs) = index.by_name.get(w.as_str()) else { continue };
        let mut defs = defs.clone();
        defs.sort_by(|a, b| index.sym_rank[*b as usize].partial_cmp(&index.sym_rank[*a as usize]).unwrap_or(std::cmp::Ordering::Equal));
        for &s in defs.iter().take(3) {
            let sym = &index.symbols[s as usize];
            let file = &index.files[sym.file as usize].path;
            let callers = &index.sym_in[s as usize];
            out.push_str(&format!("- {} `{}` defined at {}:{} (module {}); {} caller{}",
                sym.kind.as_str(), sym.qualified(), file, sym.start, module_of(index, sym.file), callers.len(), if callers.len() == 1 { "" } else { "s" }));
            let sites: Vec<String> = callers
                .iter()
                .take(4)
                .map(|&e| {
                    let e = &index.sym_edges[e as usize];
                    let from = &index.symbols[e.from as usize];
                    format!("{} ({}:{})", from.qualified(), index.files[from.file as usize].path, e.line)
                })
                .collect();
            if !sites.is_empty() {
                out.push_str(&format!(": {}", sites.join(", ")));
                if callers.len() > sites.len() {
                    out.push_str(", …");
                }
            }
            out.push('\n');
            shown += 1;
        }
        if defs.len() > 3 {
            out.push_str(&format!("  ({} more definitions named `{w}`)\n", defs.len() - 3));
        }
    }
    if shown == 0 {
        // No exact symbol: point at the best-matching code instead.
        let q = words.join(" ");
        let r = index.search(&q, &SearchOptions { limit: 3, snippet_lines: 0, ..Default::default() });
        for h in r.hits {
            let label = h.symbol.as_ref().map(|s| format!("{} {}", s.kind, s.name)).unwrap_or_else(|| "text".into());
            out.push_str(&format!("- likely relevant: {}:{}-{} {}\n", h.file, h.start, h.end, label));
        }
    }
    (!out.is_empty()).then_some(out)
}

fn glob_context(index: &Index, words: &[String]) -> Option<String> {
    let mut out = String::new();
    for w in words {
        let lw = w.to_ascii_lowercase();
        let mut hits: Vec<u32> = index
            .code_files()
            .filter(|(_, f)| {
                let name = f.path.rsplit('/').next().unwrap_or(&f.path).to_ascii_lowercase();
                name.contains(&lw)
            })
            .map(|(i, _)| i)
            .collect();
        hits.sort_by(|a, b| index.file_rank[*b as usize].partial_cmp(&index.file_rank[*a as usize]).unwrap_or(std::cmp::Ordering::Equal));
        for &f in hits.iter().take(5) {
            let entry = &index.files[f as usize];
            out.push_str(&format!("- {} (module {}, {} symbols, used by {} files)\n", entry.path, module_of(index, f), index.file_symbols(f).len(), index.file_in[f as usize].len()));
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_words() {
        assert_eq!(words(r"\bverifyToken\("), vec!["verifyToken"]);
        assert_eq!(words(r"function\s+handleRefresh"), vec!["handleRefresh"]);
        assert_eq!(words("**/*session*.ts"), vec!["session"]);
        assert!(words(".*").is_empty());
    }

    #[test]
    fn answers_for_fixture() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../repomap-core/tests/fixture");
        let index = Index::build(&root, &BuildOptions { use_cache: false }).unwrap();
        let g = grep_context(&index, &["verifyToken".into()]).unwrap();
        assert!(g.contains("src/auth/token.ts:7"), "{g}");
        assert!(g.contains("SessionStore.refresh"), "{g}");
        let f = glob_context(&index, &["session".into()]).unwrap();
        assert!(f.contains("src/auth/session.ts"), "{f}");
    }
}
