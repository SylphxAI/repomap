use repomap_core::{BuildOptions, ContextOptions, ImpactOptions, Index, MapOptions, SearchOptions, TraceOptions};
use std::path::PathBuf;

fn fixture() -> Index {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture");
    Index::build(&root, &BuildOptions { use_cache: false, ..Default::default() }).expect("index")
}

#[test]
fn map_lists_modules_and_symbols() {
    let idx = fixture();
    let m = idx.map(&MapOptions { focus: None, limit: 10 });
    assert_eq!(m.code_files, 4);
    assert!(m.symbols >= 7, "{}", m.symbols);
    assert!(m.text().contains("token.ts"));
}

#[test]
fn map_relative_focus_preserves_scope_and_budget() {
    let idx = fixture();
    let expected = idx.map(&MapOptions { focus: Some("src/auth".into()), limit: 10 });
    assert_eq!(expected.code_files, 2);
    assert!(expected.key_files.iter().all(|f| f.path.starts_with("src/auth/")));
    for focus in ["./src/auth", "././src/auth/"] {
        let mut map = idx.map(&MapOptions { focus: Some(focus.into()), limit: 10 });
        assert_eq!(serde_json::to_value(&map).unwrap(), serde_json::to_value(&expected).unwrap(), "{focus}");
        map.fit_tokens(150);
        assert!(map.key_files.iter().all(|f| f.path.starts_with("src/auth/")));
        assert!(repomap_core::query::estimate_tokens(&map.text()) <= 150);
    }
    for focus in ["./src/auth-missing", "./missing", "./../src/auth"] {
        let map = idx.map(&MapOptions { focus: Some(focus.into()), limit: 10 });
        assert_eq!(map.code_files, 0, "{focus}");
        assert!(map.key_files.is_empty() && map.key_symbols.is_empty());
    }
    let whole = idx.map(&MapOptions { focus: None, limit: 10 });
    let dot = idx.map(&MapOptions { focus: Some("./".into()), limit: 10 });
    assert_eq!(serde_json::to_value(dot).unwrap(), serde_json::to_value(whole).unwrap());
}

#[test]
fn search_finds_symbol_and_text() {
    let idx = fixture();
    let r = idx.search("verifyToken", &SearchOptions::default());
    assert_eq!(r.hits[0].symbol.as_ref().unwrap().name, "verifyToken");
    let r = idx.search("token expired", &SearchOptions::default());
    assert_eq!(r.hits[0].file, "src/auth/token.ts");
}

#[test]
fn search_reads_identifiers_and_paths_out_of_a_report() {
    let idx = fixture();
    let opts = SearchOptions { limit: 5, ..SearchOptions::default() };
    // The report names a method; the file is only in a stack trace.
    let r = idx.search("Login keeps the old credentials\n\nAfter login `SessionStore.refresh` returns the same thing every time and nothing changes for the user.", &opts);
    assert_eq!(r.hits[0].file, "src/auth/session.ts", "{:?}", r.hits.iter().map(|h| &h.file).collect::<Vec<_>>());
    let r = idx.search("Something odd happens when requests come in. Traceback: at handle (src/api/router.ts:12)", &opts);
    assert_eq!(r.hits[0].file, "src/api/router.ts", "{:?}", r.hits.iter().map(|h| &h.file).collect::<Vec<_>>());
}

#[test]
fn report_search_keeps_path_and_kind_filters() {
    let idx = fixture();
    let opts = SearchOptions {
        limit: 20,
        path: Some("src/auth/token.ts".to_string()),
        kind: Some("function".to_string()),
        include_tests: false,
        ..SearchOptions::default()
    };
    // Named paths and identifier candidates must not escape the caller's filters.
    let r = idx.search("Refreshing `SessionStore.refresh` fails in src/auth/session.ts, after verifyToken reports token expired", &opts);
    assert!(!r.hits.is_empty());
    assert!(r.hits.iter().all(|h| h.file == "src/auth/token.ts"));
    assert!(r.hits.iter().all(|h| h.symbol.as_ref().is_some_and(|s| s.kind == "function")));
}

#[test]
fn context_shows_callers() {
    let idx = fixture();
    let c = idx.context("verifyToken", &ContextOptions::default()).unwrap();
    assert!(c.callers.iter().any(|s| s.symbol.name == "SessionStore.refresh"), "{:?}", c.callers);
    assert!(c.callees.iter().any(|s| s.symbol.name == "decode"));
    let f = idx.context("src/auth/token.ts", &ContextOptions::default()).unwrap();
    assert!(f.imported_by.contains(&"src/auth/session.ts".to_string()));
}

#[test]
fn trace_finds_call_path() {
    let idx = fixture();
    let t = idx
        .trace("handleRefresh", Some("decode"), &TraceOptions { direction: repomap_core::Direction::Callees, depth: 3 })
        .unwrap();
    assert!(t.found);
    assert_eq!(t.level, "calls");
    assert_eq!(t.hops.len(), 3, "{:?}", t.hops);
}

#[test]
fn impact_reaches_tests() {
    let idx = fixture();
    let r = idx.impact(&["decode".to_string()], &ImpactOptions { depth: 4, limit: 30 }).unwrap();
    let names: Vec<String> = r.by_depth.iter().flatten().map(|s| s.symbol.name.clone()).collect();
    assert!(names.contains(&"verifyToken".to_string()));
    assert!(names.contains(&"handleRefresh".to_string()), "{names:?}");
    assert!(r.tests.contains(&"tests/session.test.ts".to_string()), "{:?}", r.tests);
}

#[test]
fn graph_json_is_complete() {
    let idx = fixture();
    let g = idx.graph_json(&Default::default(), "test");
    assert_eq!(g["nodes"].as_array().unwrap().len(), 4);
    assert!(!g["edges"].as_array().unwrap().is_empty());
}

#[test]
fn map_token_budget_trims_by_rank_and_default_is_unchanged() {
    use repomap_core::query::estimate_tokens;
    // The test fixture is the small repository; this workspace's `crates` is the medium one.
    let small = fixture();
    let medium_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let medium = Index::build(&medium_root, &BuildOptions { use_cache: false, ..Default::default() }).expect("index");
    for idx in [&small, &medium] {
        let base = idx.map(&MapOptions { focus: None, limit: 500 });
        let full = base.text();
        // Unbudgeted output carries no note, and a budget that is large enough changes nothing.
        assert!(!full.contains("(budget"));
        let mut same = idx.map(&MapOptions { focus: None, limit: 500 });
        same.fit_tokens(usize::MAX);
        assert_eq!(same.text(), full);
        let total = estimate_tokens(&full);
        for b in [150usize, 400, 1000, 2000] {
            if b >= total {
                continue;
            }
            let mut m = idx.map(&MapOptions { focus: None, limit: 500 });
            m.fit_tokens(b);
            let t = m.text();
            let got = estimate_tokens(&t);
            assert!(t.contains("omitted (budget"), "{t}");
            // Items are cut one at a time (about 30 tokens each), so the +-5% bound holds from 1,000 up.
            if got > b {
                assert!(m.key_files.is_empty() && m.key_symbols.is_empty() && m.modules.is_empty(), "{got} > {b}");
            } else if b >= 1000 {
                assert!(got as f64 >= b as f64 * 0.95, "{got} under 95% of {b}");
            }
            // Survivors are a rank-order prefix of the unbudgeted lists.
            let full_files: Vec<_> = base.key_files.iter().map(|f| &f.path).collect();
            let kept: Vec<_> = m.key_files.iter().map(|f| &f.path).collect();
            assert_eq!(kept, full_files[..kept.len()].to_vec());
            let full_syms: Vec<_> = base.key_symbols.iter().map(|s| (&s.symbol.name, &s.symbol.file)).collect();
            let kept: Vec<_> = m.key_symbols.iter().map(|s| (&s.symbol.name, &s.symbol.file)).collect();
            assert_eq!(kept, full_syms[..kept.len()].to_vec());
        }
    }
}

#[test]
fn map_budget_on_an_empty_repo_has_no_empty_note() {
    let dir = std::env::temp_dir().join(format!("repomap-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let idx = Index::build(&dir, &BuildOptions { use_cache: false, ..Default::default() }).expect("index");
    let mut m = idx.map(&MapOptions { focus: None, limit: 500 });
    m.fit_tokens(5);
    assert!(m.budget.is_none());
    assert!(!m.text().contains("omitted"));
    assert!(serde_json::to_value(&m).unwrap().get("budget").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn map_budget_on_a_huge_focused_map_is_linear_and_exact() {
    use repomap_core::query::{estimate_tokens, KeyFile, Outline};
    let idx = fixture();
    let mut m = idx.map(&MapOptions { focus: None, limit: 10 });
    m.key_files = (0..20_000).map(|i| KeyFile { path: format!("src/dir{}/file{i}.rs", i % 50), rank: 0.1, symbols: 3, imported_by: 2 }).collect();
    m.outline = (0..20_000)
        .map(|i| Outline { path: format!("src/f{i}.rs"), symbols: (0..30).map(|j| (format!("sym{j}"), "function", j, "fn sym()".to_string())).collect() })
        .collect();
    let start = std::time::Instant::now();
    m.fit_tokens(2000);
    // A rebuild per removed item (600k+ items over a ~30 MB text) would take minutes.
    assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
    let got = estimate_tokens(&m.text());
    assert!(got <= 2000 && got as f64 >= 2000.0 * 0.95, "{got}");
}

fn git_in(dir: &std::path::Path, args: &[&str]) {
    let s = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args)
        .status()
        .expect("git");
    assert!(s.success(), "git {args:?}");
}

#[test]
fn impact_changed_rejects_option_like_base_and_accepts_real_refs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir(&root).unwrap();
    git_in(&root, &["init", "-q", "-b", "main"]);
    std::fs::write(root.join("a.ts"), "export function a() { return 1; }\n").unwrap();
    git_in(&root, &["add", "."]);
    git_in(&root, &["commit", "-q", "-m", "one"]);
    std::fs::write(root.join("a.ts"), "export function a() { return 2; }\n").unwrap();
    git_in(&root, &["commit", "-q", "-am", "two"]);
    // An uncommitted edit, so every ref below has a non-empty diff.
    std::fs::write(root.join("a.ts"), "export function a() { return 3; }\n").unwrap();
    let sha = String::from_utf8(std::process::Command::new("git").arg("-C").arg(&root).args(["rev-parse", "HEAD~1"]).output().unwrap().stdout).unwrap();
    let idx = Index::build(&root, &BuildOptions { use_cache: false, ..Default::default() }).expect("index");
    let opts = ImpactOptions::default();

    let victim = dir.path().join("pwned");
    for bad in [format!("--output={}", victim.display()), "-p".to_string(), "--".to_string()] {
        let e = idx.impact_changed(Some(&bad), &opts).expect_err("must reject");
        assert!(e.contains("must not start with `-`"), "{e}");
    }
    assert!(!victim.exists(), "base must not create a file");

    for ok in ["HEAD", "HEAD~1", "main", sha.trim()] {
        idx.impact_changed(Some(ok), &opts).unwrap_or_else(|e| panic!("{ok}: {e}"));
    }
    // A remote-tracking ref also resolves.
    git_in(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    idx.impact_changed(Some("origin/main"), &opts).expect("origin/main");
    // The diff against HEAD~1 sees the edited function.
    let r = idx.impact_changed(Some("HEAD~1"), &opts).unwrap();
    assert!(!r.targets.is_empty());
}
