//! Architecture Reader MCP — Rust evidence graph engine.

pub mod engine;
pub mod git;
pub mod scanner;
pub mod store;
pub mod synth;
pub mod synth_probe;
pub mod types;

pub use engine::handle_tool;
pub use types::{
    ArchitectureGraph, Confidence, ENGINE_NAME, ENGINE_VERSION, EvidenceRef, Freshness,
    ToolEnvelope, GRAPH_SCHEMA_VERSION,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    fn fixture_index_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/sample-repo")
            .canonicalize()
            .expect("fixture repo")
    }

    #[test]
    fn serializes_graph_snapshot_deterministically() {
        let root = fixture_root();
        let git = git::read_git_state(&root);
        let graph = scanner::scan_repository(&root, &scanner::ScanOptions::default(), git.commit, git.dirty);
        let a = serde_json::to_string(&graph).expect("serialize");
        let b = serde_json::to_string(&graph).expect("serialize");
        assert_eq!(a, b);
        assert!(!graph.nodes.is_empty());
        assert!(graph.nodes.iter().any(|n| n.kind == "package"));
    }

    #[test]
    fn indexes_fixture_and_returns_spec_envelope() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let input = serde_json::json!({ "root": root.to_string_lossy(), "mode": "full" });
        let envelope = engine::handle_tool("architecture_index", input);
        assert_eq!(envelope.status, "ok");
        assert!(envelope.answer.is_some());
        assert!(envelope.metrics.unwrap().node_count > 0);
    }

    #[test]
    fn extracts_routes_and_schemas_from_fixture() {
        let root = fixture_root();
        let git = git::read_git_state(&root);
        let graph = scanner::scan_repository(&root, &scanner::ScanOptions::default(), git.commit, git.dirty);

        let routes: Vec<_> = graph.nodes.iter().filter(|n| n.kind == "route").collect();
        assert!(
            routes.iter().any(|n| n.label.contains("POST") && n.label.contains("/api/auth/login")),
            "expected login route, got: {:?}",
            routes.iter().map(|n| &n.label).collect::<Vec<_>>()
        );
        assert!(
            routes.iter().any(|n| n.label.contains("GET") && n.label.contains("/health")),
            "expected health route"
        );

        let schemas: Vec<_> = graph.nodes.iter().filter(|n| n.kind == "schema").collect();
        assert!(
            schemas.iter().any(|n| n.label == "LoginRequest" || n.label == "User"),
            "expected schema nodes, got: {:?}",
            schemas.iter().map(|n| &n.label).collect::<Vec<_>>()
        );
        assert!(graph.extractors.iter().any(|e| e.starts_with("routes@")));
        assert!(graph.extractors.iter().any(|e| e.starts_with("schema@")));
    }

    #[test]
    fn status_no_longer_reports_route_or_schema_gaps_after_index() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let index_input = serde_json::json!({ "root": root.to_string_lossy(), "mode": "full" });
        let _ = engine::handle_tool("architecture_index", index_input);
        let status_input = serde_json::json!({ "root": root.to_string_lossy() });
        let envelope = engine::handle_tool("architecture_status", status_input);
        assert_eq!(envelope.status, "ok");
        let gaps = envelope.gaps;
        assert!(!gaps.iter().any(|g| g.contains("Route extraction")));
        assert!(!gaps.iter().any(|g| g.contains("Schema extraction")));
    }

    #[test]
    fn indexes_fixture_with_synth_extractor_when_probe_is_available() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let graph = scanner::scan_repository(
            &root,
            &scanner::ScanOptions {
                use_synth: true,
                ..scanner::ScanOptions::default()
            },
            None,
            false,
        );

        if graph
            .extractors
            .iter()
            .any(|extractor| extractor.starts_with("synth-"))
        {
            assert!(graph.evidence.iter().any(|ev| ev.extractor.starts_with("synth-")));
            assert!(graph.nodes.iter().any(|node| node.kind == "symbol"));
        }
    }

    #[test]
    fn auto_mode_returns_cache_hit_when_inventory_is_unchanged() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let full_input = serde_json::json!({ "root": root.to_string_lossy(), "mode": "full" });
        let first = engine::handle_tool("architecture_index", full_input.clone());
        assert_eq!(first.status, "ok");
        assert_eq!(first.answer.as_ref().and_then(|a| a.get("refreshMode")).and_then(|v| v.as_str()), Some("full"));

        let auto_input = serde_json::json!({ "root": root.to_string_lossy(), "mode": "auto" });
        let second = engine::handle_tool("architecture_index", auto_input);
        assert_eq!(second.status, "ok");
        assert_eq!(
            second.answer.as_ref().and_then(|a| a.get("refreshMode")).and_then(|v| v.as_str()),
            Some("cache_hit")
        );
    }

    #[test]
    fn omitted_and_refresh_modes_follow_the_same_branch_as_auto() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let root_str = root.to_string_lossy().to_string();
        let refresh_mode = |envelope: &ToolEnvelope| {
            envelope
                .answer
                .as_ref()
                .and_then(|answer| answer.get("refreshMode"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        };

        let full = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root_str, "mode": "full" }),
        );
        assert_eq!(full.status, "ok");
        assert_eq!(refresh_mode(&full).as_deref(), Some("full"));

        let omitted = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root_str }),
        );
        let refresh = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root_str, "mode": "refresh" }),
        );
        let auto = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root_str, "mode": "auto" }),
        );
        assert_eq!(omitted.status, "ok");
        assert_eq!(refresh.status, "ok");
        assert_eq!(auto.status, "ok");
        assert_eq!(refresh_mode(&omitted).as_deref(), Some("cache_hit"));
        assert_eq!(refresh_mode(&refresh).as_deref(), Some("cache_hit"));
        assert_eq!(refresh_mode(&auto).as_deref(), Some("cache_hit"));
        assert_ne!(refresh_mode(&refresh).as_deref(), Some("refresh"));
        assert_ne!(refresh_mode(&auto).as_deref(), Some("auto"));

        let forced = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root_str, "mode": "rebuild" }),
        );
        assert_eq!(forced.status, "ok");
        assert_eq!(refresh_mode(&forced).as_deref(), Some("full"));

        let status_only = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root_str, "mode": "status_only" }),
        );
        assert_eq!(status_only.status, "ok");
        assert_eq!(
            status_only
                .answer
                .as_ref()
                .and_then(|answer| answer.get("mode"))
                .and_then(|value| value.as_str()),
            Some("status_only")
        );
        assert!(status_only
            .answer
            .as_ref()
            .and_then(|answer| answer.get("refreshMode"))
            .is_none());
    }

    #[test]
    fn missing_index_hint_names_refresh_before_auto() {
        let dir = std::env::temp_dir().join(format!(
            "spine-no-index-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp repo");
        let envelope = engine::handle_tool(
            "architecture_status",
            serde_json::json!({ "root": dir.to_string_lossy() }),
        );
        assert_eq!(envelope.code.as_deref(), Some("INDEX_NOT_FOUND"));
        let hint = envelope.next_action.clone().unwrap_or_default();
        let refresh_at = hint.find("refresh").expect("hint names refresh");
        let auto_at = hint.find("auto").expect("hint keeps auto");
        assert!(refresh_at < auto_at, "hint was: {hint}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn indexes_python_imports_and_call_edges() {
        let root = fixture_root();
        let git = git::read_git_state(&root);
        let graph = scanner::scan_repository(&root, &scanner::ScanOptions::default(), git.commit, git.dirty);

        assert!(graph.nodes.iter().any(|node| {
            node.kind == "module" && node.path.as_deref() == Some("src/ml/scorer.py")
        }));
        assert!(graph.edges.iter().any(|edge| {
            edge.kind == "imports"
                && edge.from.contains("scorer.py")
                && edge.to.contains("auth.token")
        }));
        assert!(graph.extractors.iter().any(|extractor| extractor.starts_with("python@")));
    }

    #[test]
    fn trace_finds_symbol_call_path_in_fixture() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let index_input = serde_json::json!({
            "root": root.to_string_lossy(),
            "mode": "full",
            "useSynth": false
        });
        let _ = engine::handle_tool("architecture_index", index_input);
        let trace_input = serde_json::json!({
            "root": root.to_string_lossy(),
            "from": "authMiddleware",
            "to": "validateToken",
            "relation": "calls",
            "maxDepth": 4
        });
        let envelope = engine::handle_tool("architecture_trace", trace_input);
        assert_eq!(envelope.status, "ok");
        let path = envelope
            .answer
            .as_ref()
            .and_then(|answer| answer.get("path"))
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(
            path.len() >= 2,
            "expected symbol call trace path, got {:?}",
            path
        );
    
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("hops").is_some(), "trace must include hops provenance");
        assert!(answer.get("hopCount").is_some());
        assert_eq!(answer["fromId"].as_str().is_some(), true);
    }

    #[test]
    fn search_finds_auth_module_in_fixture() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let index_input = serde_json::json!({ "root": root.to_string_lossy(), "mode": "full" });
        let _ = engine::handle_tool("architecture_index", index_input);
        let search_input = serde_json::json!({
            "root": root.to_string_lossy(),
            "query": "auth",
            "limit": 5
        });
        let envelope = engine::handle_tool("architecture_search", search_input);
        assert_eq!(envelope.status, "ok");
        let answer = envelope.answer.expect("answer");
        let matches = answer["matches"].as_array().expect("matches array");
        assert!(!matches.is_empty());
    }

    #[test]
    fn path_returns_hops_with_provenance_in_fixture() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let index_input = serde_json::json!({
            "root": root.to_string_lossy(),
            "mode": "full",
            "useSynth": false
        });
        let _ = engine::handle_tool("architecture_index", index_input);
        let path_input = serde_json::json!({
            "root": root.to_string_lossy(),
            "from": "authMiddleware",
            "to": "validateToken",
            "relation": "calls",
            "maxDepth": 6
        });
        let envelope = engine::handle_tool("architecture_path", path_input);
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("hopCount").is_some());
        assert!(answer.get("hops").is_some());
        assert!(answer.get("nodes").is_some());
        let hops = answer["hops"].as_array().cloned().unwrap_or_default();
        if !hops.is_empty() {
            assert!(hops[0].get("provenance").is_some());
            assert!(hops[0].get("edgeKind").is_some());
            let nodes = answer["nodes"].as_array().cloned().unwrap_or_default();
            assert!(nodes.len() >= 2);
        }
    
        if let Some(hops) = answer.get("hops").and_then(|v| v.as_array()) {
            if let Some(h) = hops.first() {
                assert!(h.get("fromNode").is_some(), "hop missing fromNode: {:?}", h);
            }
        }
    }

    #[test]
    fn path_includes_mermaid_diagram() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_path",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "from": "authMiddleware",
                "to": "validateToken",
                "relation": "calls",
                "maxDepth": 6
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let mermaid = answer.get("mermaid").and_then(|v| v.as_str()).unwrap_or("");
        assert!(mermaid.contains("graph LR"), "mermaid={mermaid}");
    }

    #[test]
    fn impact_includes_mermaid_diagrams() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_impact",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "changedPaths": ["src/auth/token.ts"],
                "maxDepth": 2
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let mermaid = answer.get("mermaid").expect("mermaid");
        assert!(mermaid.get("incoming").and_then(|v| v.as_str()).unwrap_or("").contains("graph LR"));
        assert!(mermaid.get("outgoing").and_then(|v| v.as_str()).unwrap_or("").contains("graph LR"));
    }

    #[test]
    fn overview_focus_includes_mermaid_when_neighbors_exist() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_overview",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "focus": "src/auth/token.ts",
                "depth": 2
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let neighbors = answer.get("neighbors").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if !neighbors.is_empty() {
            let mermaid = answer.get("mermaid").and_then(|v| v.as_str()).unwrap_or("");
            assert!(mermaid.contains("graph LR"), "mermaid={mermaid}");
        }
    }


    #[test]
    fn context_pack_includes_mermaid() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_context_pack",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "focus": "src/auth/token.ts",
                "maxNeighbors": 8
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let mermaid = answer.get("mermaid").and_then(|v| v.as_str()).unwrap_or("").to_string();
        assert!(mermaid.contains("graph LR"), "mermaid={mermaid}");
    }


    #[test]
    fn impact_reports_direct_nodes_for_changed_path() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_impact",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "changedPaths": ["src/auth/token.ts"]
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert_eq!(answer["changedPathSource"], "explicit");
        let direct = answer["directImpact"].as_array().cloned().unwrap_or_default();
        assert!(
            !direct.is_empty(),
            "expected direct impact for auth token module, got {:?}",
            answer
        );
    }

    #[test]
    fn impact_use_git_diff_sets_source_git() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_impact",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "useGitDiff": true
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert_eq!(answer["changedPathSource"], "git");
        assert!(answer.get("changedPaths").is_some());
    }



    #[test]
    fn indexes_rust_and_go_fixtures() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        // ensure fixture files present
        assert!(root.join("src/rust/token.rs").exists());
        assert!(root.join("src/go/auth/token.go").exists());
        let envelope = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let overview = engine::handle_tool(
            "architecture_search",
            serde_json::json!({ "root": root.to_string_lossy(), "query": "issue_token" }),
        );
        assert_eq!(overview.status, "ok", "{:?}", overview.message);
        let answer = overview.answer.expect("answer");
        let matches = answer["matches"].as_array().cloned().unwrap_or_default();
        assert!(
            matches.iter().any(|m| m["id"].as_str().unwrap_or("").contains("issue_token")
                || m["label"].as_str().unwrap_or("").contains("issue_token")),
            "expected issue_token match, got {:?}",
            matches
        );
        let go = engine::handle_tool(
            "architecture_search",
            serde_json::json!({ "root": root.to_string_lossy(), "query": "IssueToken" }),
        );
        assert_eq!(go.status, "ok");
        let gans = go.answer.expect("answer");
        let gmatches = gans["matches"].as_array().cloned().unwrap_or_default();
        assert!(
            gmatches.iter().any(|m| m["label"].as_str().unwrap_or("").contains("IssueToken")
                || m["id"].as_str().unwrap_or("").contains("IssueToken")),
            "expected IssueToken match {:?}",
            gmatches
        );
    }


    #[test]
    fn search_ranks_exact_symbol_above_path_substring() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_search",
            serde_json::json!({ "root": root.to_string_lossy(), "query": "issue_token", "limit": 10 }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let matches = envelope.answer.expect("answer")["matches"].as_array().cloned().unwrap_or_default();
        assert!(!matches.is_empty());
        let top = &matches[0];
        assert!(
            top["label"].as_str() == Some("issue_token") || top["id"].as_str().unwrap_or("").contains("issue_token"),
            "top match should prefer exact symbol, got {:?}",
            top
        );
        let scores: Vec<f64> = matches.iter().filter_map(|m| m["score"].as_f64()).collect();
        assert!(scores.windows(2).all(|w| w[0] >= w[1]), "scores should be non-increasing: {:?}", scores);

        let overview = engine::handle_tool(
            "architecture_overview",
            serde_json::json!({ "root": root.to_string_lossy(), "depth": 3 }),
        );
        assert_eq!(overview.status, "ok");
        let ans = overview.answer.expect("answer");
        assert!(ans.get("counts").is_some());
        assert!(ans.get("languages").is_some());
        assert!(ans.get("extractors").is_some());
    }


    #[test]
    fn impact_reports_incoming_dependents() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        // token.ts is imported by middleware / routes in fixture — expect reverse edges when present
        let envelope = engine::handle_tool(
            "architecture_impact",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "changedPaths": ["src/auth/token.ts"],
                "maxDepth": 2
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("incomingImpact").is_some());
        assert!(answer.get("outgoingImpact").is_some());
        assert_eq!(answer["maxDepth"], 2);
        let direct = answer["directImpact"].as_array().cloned().unwrap_or_default();
        assert!(!direct.is_empty(), "expected direct impact nodes");
        // At least one of incoming/outgoing should be non-empty on this fixture
        let inc = answer["incomingImpact"].as_array().cloned().unwrap_or_default();
        let out = answer["outgoingImpact"].as_array().cloned().unwrap_or_default();
        assert!(
            !inc.is_empty() || !out.is_empty(),
            "expected blast-radius edges, answer={:?}",
            answer
        );
    }


    #[test]
    fn overview_returns_neighbors_for_focus() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_overview",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "focus": "src/auth/token.ts",
                "depth": 3
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("focusNodeId").is_some());
        let neighbors = answer["neighbors"].as_array().cloned().unwrap_or_default();
        assert!(
            !neighbors.is_empty(),
            "expected neighbors for token.ts focus, answer={:?}",
            answer
        );
    }


    #[test]
    fn search_score_explain_and_neighbors() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_search",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "query": "issue_token",
                "limit": 5,
                "includeNeighbors": true
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert_eq!(answer["includeNeighbors"], true);
        let matches = answer["matches"].as_array().cloned().unwrap_or_default();
        assert!(!matches.is_empty());
        let top = &matches[0];
        let explain = top["scoreExplain"].as_array().cloned().unwrap_or_default();
        assert!(
            explain.iter().any(|e| e.as_str() == Some("exact_label") || e.as_str() == Some("label_substring") || e.as_str() == Some("label_prefix")),
            "scoreExplain={:?}",
            explain
        );
        assert!(top.get("neighbors").is_some(), "expected neighbors on match");
    }


    #[test]
    fn overview_includes_cycles_field() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_overview",
            serde_json::json!({ "root": root.to_string_lossy(), "depth": 2 }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("cycles").is_some(), "cycles field required");
        assert!(answer["cycles"].is_array());
    }


    #[test]
    fn impact_unknown_path_is_honest() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_impact",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "changedPaths": ["src/auth/token.ts", "does/not/exist.ts"],
                "maxDepth": 1
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let unknown = answer["unknownImpact"].as_array().cloned().unwrap_or_default();
        assert!(
            unknown.iter().any(|u| u["path"] == "does/not/exist.ts"),
            "expected unknownImpact for missing path, got {:?}",
            unknown
        );
        let overview = engine::handle_tool(
            "architecture_overview",
            serde_json::json!({ "root": root.to_string_lossy(), "depth": 2 }),
        );
        assert_eq!(overview.status, "ok");
        let oans = overview.answer.expect("answer");
        assert!(oans.get("topFanIn").is_some());
        assert!(oans["topFanIn"].is_array());
    }


    #[test]
    fn status_reports_languages_and_fanin() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_status",
            serde_json::json!({ "root": root.to_string_lossy() }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("languages").is_some());
        assert!(answer.get("topFanIn").is_some());
        assert!(answer.get("topFanOut").is_some());
        assert!(answer.get("cycles").is_some());
        assert!(answer.get("defaultExcludes").is_some());
        assert!(answer["defaultExcludes"].as_array().unwrap().iter().any(|v| v.as_str() == Some("node_modules")));
        assert!(answer["coverage"]["evidence"].as_u64().unwrap_or(0) > 0);
    }


    #[test]
    fn path_suggestions_for_unresolved() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_path",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "from": "auth",
                "to": "totally-missing-entity-xyz",
                "maxDepth": 4
            }),
        );
        // may be ok with gaps when one side resolves partially via label
        let answer = envelope.answer.expect("answer");
        let suggestions = answer.get("suggestions").cloned().unwrap_or(serde_json::json!({}));
        assert!(
            suggestions.get("to").is_some() || envelope.status == "ok",
            "expected suggestions or ok envelope, got status={:?} answer={:?}",
            envelope.status,
            answer
        );
        // force unresolved end
        let envelope2 = engine::handle_tool(
            "architecture_path",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "from": "___no_such_from___",
                "to": "___no_such_to___",
            }),
        );
        let answer2 = envelope2.answer.expect("answer");
        let sug2 = answer2["suggestions"].as_object().cloned().unwrap_or_default();
        assert!(sug2.contains_key("from") || sug2.contains_key("to") || answer2.get("fromId").is_none());
    }

    #[test]
    fn impact_edges_include_node_summaries() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_impact",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "changedPaths": ["src/auth/token.ts"],
                "maxDepth": 2
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let out = answer["outgoingImpact"].as_array().cloned().unwrap_or_default();
        let inc = answer["incomingImpact"].as_array().cloned().unwrap_or_default();
        let sample = out.into_iter().chain(inc).next();
        if let Some(edge) = sample {
            assert!(edge.get("fromNode").is_some() || edge.get("toNode").is_some(), "{:?}", edge);
        }
    }


    #[test]
    fn evidence_resolves_node_and_reports_missing() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let search = engine::handle_tool(
            "architecture_search",
            serde_json::json!({ "root": root.to_string_lossy(), "query": "token", "limit": 3 }),
        );
        assert_eq!(search.status, "ok");
        let mid = search
            .answer
            .as_ref()
            .and_then(|a| a["matches"].as_array())
            .and_then(|m| m.first())
            .and_then(|m| m["id"].as_str())
            .expect("match id")
            .to_string();
        let envelope = engine::handle_tool(
            "architecture_evidence",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "ids": [mid, "ev_does_not_exist"]
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer["items"].as_u64().unwrap_or(0) >= 1 || !answer["resolved"].as_array().unwrap().is_empty());
        let missing = answer["missing"].as_array().cloned().unwrap_or_default();
        assert!(
            missing.iter().any(|m| m.as_str() == Some("ev_does_not_exist")),
            "missing={:?}",
            missing
        );
    }


    #[test]
    fn index_answer_includes_languages() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let envelope = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("languages").is_some());
        assert!(answer.get("extractors").is_some());
        assert!(answer["coverage"]["symbols"].as_u64().unwrap_or(0) > 0);
    }


    #[test]
    fn search_types_filter_symbols() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_search",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "query": "token",
                "types": ["symbol"],
                "limit": 20
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let matches = envelope.answer.expect("answer")["matches"].as_array().cloned().unwrap_or_default();
        assert!(!matches.is_empty());
        assert!(
            matches.iter().all(|m| m["kind"] == "symbol"),
            "expected only symbols, got {:?}",
            matches
        );
    }


    #[test]
    fn index_exclude_extends_defaults_and_echoes_scan() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let envelope = engine::handle_tool(
            "architecture_index",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "mode": "full",
                "useSynth": false,
                "exclude": ["custom_noise"]
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        let exclude = answer["scan"]["exclude"].as_array().cloned().unwrap_or_default();
        let as_str: Vec<_> = exclude.iter().filter_map(|v| v.as_str()).collect();
        assert!(as_str.iter().any(|e| *e == "node_modules"), "defaults retained: {:?}", as_str);
        assert!(as_str.iter().any(|e| *e == "custom_noise"), "custom exclude present: {:?}", as_str);
        assert!(as_str.iter().any(|e| *e == ".next"), "expanded defaults present: {:?}", as_str);
    }


    #[test]
    fn search_empty_query_browse_fan_in() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_search",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "query": "",
                "limit": 5
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let matches = envelope.answer.expect("answer")["matches"].as_array().cloned().unwrap_or_default();
        assert!(!matches.is_empty());
        let explain = matches[0]["scoreExplain"].as_array().cloned().unwrap_or_default();
        assert!(
            explain.iter().any(|e| e.as_str() == Some("browse_fan_in") || e.as_str() == Some("empty_query")),
            "expected browse ranking, got {:?}",
            explain
        );
    }

    #[test]
    fn search_empty_browse_skips_repository_and_explains_fan_in() {
        let root = std::env::temp_dir().join(format!(
            "spine-browse-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("lib.ts"),
            "export function helper() { return 1 }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("main.ts"),
            "import { helper } from './lib'\nexport const x = helper()\n",
        )
        .unwrap();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_search",
            serde_json::json!({ "root": root.to_string_lossy(), "query": "", "limit": 20 }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let matches = envelope
            .answer
            .expect("answer")["matches"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            matches
                .iter()
                .all(|m| m.get("kind").and_then(|k| k.as_str()) != Some("repository")),
            "repository nodes should be excluded from empty browse: {:?}",
            matches
        );
        let any_fan = matches.iter().any(|m| {
            m.get("scoreExplain")
                .and_then(|e| e.as_array())
                .map(|arr| arr.iter().any(|x| x.as_str().unwrap_or("").starts_with("fanIn=")))
                .unwrap_or(false)
        });
        assert!(any_fan, "expected fanIn= in scoreExplain, got {:?}", matches);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn overview_and_status_include_orphans() {
        let root = std::env::temp_dir().join(format!(
            "spine-orphans-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("used.ts"), "export const a = 1\n").unwrap();
        std::fs::write(
            root.join("entry.ts"),
            "import { a } from './used'\nexport const b = a\n",
        )
        .unwrap();
        std::fs::write(root.join("lonely.ts"), "export const z = 9\n").unwrap();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let overview = engine::handle_tool(
            "architecture_overview",
            serde_json::json!({ "root": root.to_string_lossy(), "focus": "all", "depth": 2 }),
        );
        assert_eq!(overview.status, "ok", "{:?}", overview.message);
        let overview_answer = overview.answer.expect("answer");
        let orphans = overview_answer["orphans"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            !orphans.is_empty(),
            "expected orphans list, got {:?}",
            overview_answer
        );
        let status = engine::handle_tool(
            "architecture_status",
            serde_json::json!({ "root": root.to_string_lossy() }),
        );
        assert_eq!(status.status, "ok", "{:?}", status.message);
        let status_answer = status.answer.expect("answer");
        assert!(
            status_answer
                .get("orphans")
                .and_then(|v| v.as_array())
                .is_some(),
            "status missing orphans: {:?}",
            status_answer
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn context_pack_returns_focus_neighborhood() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_context_pack",
            serde_json::json!({
                "root": root.to_string_lossy(),
                "focus": "src/auth/token.ts",
                "maxNeighbors": 8
            }),
        );
        assert_eq!(envelope.status, "ok", "{:?}", envelope.message);
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("focusNode").is_some(), "{:?}", answer);
        assert!(answer.get("neighbors").and_then(|v| v.as_array()).is_some(), "{:?}", answer);
        assert!(answer.get("evidence").is_some(), "{:?}", answer);
        let empty = engine::handle_tool(
            "architecture_context_pack",
            serde_json::json!({ "root": root.to_string_lossy(), "focus": "" }),
        );
        assert_eq!(empty.status, "error");
        assert_eq!(empty.code.as_deref(), Some("INVALID_INPUT"));
    }



    #[test]
    fn status_reports_relation_kinds() {
        let _guard = fixture_index_lock().lock().expect("fixture index lock");
        let root = fixture_root();
        let _ = engine::handle_tool(
            "architecture_index",
            serde_json::json!({ "root": root.to_string_lossy(), "mode": "full", "useSynth": false }),
        );
        let envelope = engine::handle_tool(
            "architecture_status",
            serde_json::json!({ "root": root.to_string_lossy() }),
        );
        assert_eq!(envelope.status, "ok");
        let answer = envelope.answer.expect("answer");
        assert!(answer.get("relationKinds").is_some());
    }

}
