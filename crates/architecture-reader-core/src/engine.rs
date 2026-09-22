use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::time::Instant;

use serde_json::json;

use crate::git::{freshness, list_changed_paths, read_git_state};
use crate::scanner::{
    incremental_refresh, inventory_files, scan_repository, ScanOptions,
};
use crate::store::{load_file_hashes, load_graph, save_file_hashes, save_graph, FileHashManifest};
use crate::types::{
    ArchitectureGraph, Confidence, EvidenceRef, GraphNode, Metrics, RepositoryState, ToolEnvelope,
    GRAPH_SCHEMA_VERSION,
};

pub fn handle_tool(tool: &str, input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let result = match tool {
        "architecture_index" => architecture_index(input),
        "architecture_status" => architecture_status(input),
        "architecture_overview" => architecture_overview(input),
        "architecture_explain" => architecture_explain(input),
        "architecture_search" => architecture_search(input),
        "architecture_path" => architecture_path(input),
        "architecture_trace" => architecture_trace(input),
        "architecture_impact" => architecture_impact(input),
        "architecture_evidence" => architecture_evidence(input),
        "architecture_context_pack" => architecture_context_pack(input),
        _ => ToolEnvelope::error("UNSUPPORTED_QUERY", &format!("Unknown tool: {tool}"), None),
    };
    let _ = started;
    let mut result = result;
    result.tool = Some(tool.to_string());
    // Family route path uses tool name for progressive disclosure.
    result.route = serde_json::json!({ "engine": "rust-core", "path": tool });
    result
}

fn resolve_root(input: &serde_json::Value) -> Result<std::path::PathBuf, ToolEnvelope> {
    let root = input
        .get("root")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolEnvelope::error("INVALID_ROOT", "Missing required field: root", None))?;
    let path = std::path::PathBuf::from(root);
    if !path.is_dir() {
        return Err(ToolEnvelope::error(
            "INVALID_ROOT",
            &format!("Root is not a directory: {root}"),
            None,
        ));
    }
    Ok(path.canonicalize().unwrap_or(path))
}

fn repository_state(root: &Path, graph: Option<&ArchitectureGraph>) -> RepositoryState {
    let git = read_git_state(root);
    let indexed_commit = graph.and_then(|g| g.repository.git_commit.clone());
    RepositoryState {
        root: root.to_string_lossy().to_string(),
        indexed_commit: indexed_commit.clone(),
        current_commit: git.commit.clone(),
        freshness: freshness(
            indexed_commit.as_deref(),
            git.commit.as_deref(),
            git.dirty,
        ),
        worktree_dirty: git.dirty,
    }
}

fn metrics(started: Instant, graph: &ArchitectureGraph) -> Metrics {
    Metrics {
        elapsed_ms: started.elapsed().as_millis() as u64,
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
    }
}

fn require_graph(root: &Path) -> Result<ArchitectureGraph, ToolEnvelope> {
    load_graph(root).ok_or_else(|| {
        ToolEnvelope::error(
            "INDEX_NOT_FOUND",
            "No architecture index exists for this repository.",
            Some("Call architecture_index with mode full or the compatibility alias auto."),
        )
    })
}


/// Bounded search for short directed cycles (Graphify-class structural signal).
fn find_short_cycles(graph: &ArchitectureGraph, max_cycles: usize, max_len: usize) -> Vec<serde_json::Value> {
    use std::collections::{HashMap, HashSet, VecDeque};
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    for e in &graph.edges {
        // Prefer structural import/depends edges; call edges create noisy false cycles.
        if matches!(e.kind.as_str(), "imports" | "depends_on") {
            adj.entry(e.from.clone()).or_default().push(e.to.clone());
        }
    }
    let mut cycles: Vec<serde_json::Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut starts: Vec<String> = adj.keys().cloned().collect();
    starts.sort();
    starts.truncate(120);
    for start in starts {
        let mut q: VecDeque<Vec<String>> = VecDeque::new();
        q.push_back(vec![start.clone()]);
        while let Some(path) = q.pop_front() {
            if cycles.len() >= max_cycles {
                break;
            }
            if path.len() > max_len {
                continue;
            }
            let last = path.last().cloned().unwrap_or_default();
            let Some(nexts) = adj.get(&last) else { continue };
            for next in nexts {
                if next == &start && path.len() >= 2 {
                    let mut cyc = path.clone();
                    cyc.push(start.clone());
                    let mut key_parts = path.clone();
                    if let Some(min_i) = key_parts.iter().enumerate().min_by_key(|(_, s)| *s).map(|(i, _)| i) {
                        key_parts.rotate_left(min_i);
                    }
                    let key = key_parts.join("->");
                    if seen.insert(key) {
                        cycles.push(json!({ "nodes": cyc, "length": path.len() }));
                    }
                    continue;
                }
                if path.contains(next) {
                    continue;
                }
                if path.len() < max_len {
                    let mut np = path.clone();
                    np.push(next.clone());
                    q.push_back(np);
                }
            }
        }
        if cycles.len() >= max_cycles {
            break;
        }
    }
    cycles.sort_by(|a, b| {
        let ka = a
            .get("nodes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join("->")
            })
            .unwrap_or_default();
        let kb = b
            .get("nodes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join("->")
            })
            .unwrap_or_default();
        ka.cmp(&kb)
    });
    cycles.truncate(max_cycles);
    cycles
}


fn top_fan_in_modules(graph: &ArchitectureGraph, limit: usize) -> Vec<serde_json::Value> {
    use std::collections::HashMap;
    let mut fan_in: HashMap<&str, u64> = HashMap::new();
    for e in &graph.edges {
        if matches!(e.kind.as_str(), "imports" | "calls" | "depends_on") {
            *fan_in.entry(e.to.as_str()).or_default() += 1;
        }
    }
    let mut ranked: Vec<_> = fan_in.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .into_iter()
        .take(limit.max(1))
        .map(|(id, count)| {
            let node = graph.nodes.iter().find(|n| n.id == id);
            json!({
                "id": id,
                "fanIn": count,
                "kind": node.map(|n| n.kind.clone()),
                "label": node.map(|n| n.label.clone()),
                "path": node.and_then(|n| n.path.clone()),
            })
        })
        .collect()
}


fn top_fan_out_modules(graph: &ArchitectureGraph, limit: usize) -> Vec<serde_json::Value> {
    use std::collections::HashMap;
    let mut fan_out: HashMap<&str, u64> = HashMap::new();
    for e in &graph.edges {
        if matches!(e.kind.as_str(), "imports" | "calls" | "depends_on") {
            *fan_out.entry(e.from.as_str()).or_default() += 1;
        }
    }
    let mut ranked: Vec<_> = fan_out.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ranked
        .into_iter()
        .take(limit.max(1))
        .map(|(id, count)| {
            let node = graph.nodes.iter().find(|n| n.id == id);
            json!({
                "id": id,
                "fanOut": count,
                "kind": node.map(|n| n.kind.clone()),
                "label": node.map(|n| n.label.clone()),
                "path": node.and_then(|n| n.path.clone()),
            })
        })
        .collect()
}


/// Modules/files with zero inbound structural edges (import/call/depends).
/// Graphify-class "what is unused / leaf / entry-adjacent" signal for agents.
fn orphan_modules(graph: &ArchitectureGraph, limit: usize) -> Vec<serde_json::Value> {
    use std::collections::{HashMap, HashSet};
    let mut inbound: HashSet<&str> = HashSet::new();
    let mut outbound: HashMap<&str, u64> = HashMap::new();
    for e in &graph.edges {
        if matches!(e.kind.as_str(), "imports" | "calls" | "depends_on") {
            inbound.insert(e.to.as_str());
            *outbound.entry(e.from.as_str()).or_default() += 1;
        }
    }
    let mut orphans: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| matches!(n.kind.as_str(), "module" | "file") && n.path.is_some())
        .filter(|n| !inbound.contains(n.id.as_str()))
        .map(|n| {
            let fo = *outbound.get(n.id.as_str()).unwrap_or(&0);
            (fo, n)
        })
        .collect();
    // Prefer orphans that still import others (likely leaves/utilities) then stable path order.
    orphans.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    orphans
        .into_iter()
        .take(limit.max(1))
        .map(|(fan_out, n)| {
            json!({
                "id": n.id,
                "kind": n.kind,
                "label": n.label,
                "path": n.path,
                "fanIn": 0,
                "fanOut": fan_out,
                "note": "no_inbound_imports_calls_depends",
            })
        })
        .collect()
}


fn node_summary(graph: &ArchitectureGraph, id: &str) -> serde_json::Value {
    if let Some(n) = graph.nodes.iter().find(|n| n.id == id) {
        json!({ "id": n.id, "kind": n.kind, "label": n.label, "path": n.path })
    } else {
        json!({ "id": id })
    }
}

fn suggest_nodes(graph: &ArchitectureGraph, needle: &str, limit: usize) -> Vec<serde_json::Value> {
    let q = needle.to_lowercase();
    let mut scored: Vec<(i32, &crate::types::GraphNode)> = Vec::new();
    for n in &graph.nodes {
        let label = n.label.to_lowercase();
        let path = n.path.as_deref().unwrap_or("").to_lowercase();
        let score = if label == q {
            100
        } else if label.starts_with(&q) {
            80
        } else if label.contains(&q) {
            60
        } else if path.contains(&q) {
            40
        } else if n.id.to_lowercase().contains(&q) {
            20
        } else {
            0
        };
        if score > 0 {
            scored.push((score, n));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, n)| json!({ "id": n.id, "kind": n.kind, "label": n.label, "path": n.path }))
        .collect()
}

fn architecture_index(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let mode = input.get("mode").and_then(|v| v.as_str()).unwrap_or("auto");
    let mut options = ScanOptions::default();
    if let Some(include) = input.get("include").and_then(|v| v.as_array()) {
        options.include = include
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
    }
    if let Some(exclude) = input.get("exclude").and_then(|v| v.as_array()) {
        // Extend defaults rather than replace — agents should not accidentally reindex node_modules.
        for ex in exclude.iter().filter_map(|v| v.as_str().map(str::to_string)) {
            if !options.exclude.iter().any(|e| e == &ex) {
                options.exclude.push(ex);
            }
        }
    }
    if let Some(max) = input.get("maxFileBytes").and_then(|v| v.as_u64()) {
        options.max_file_bytes = max;
    }
    if let Some(use_synth) = input.get("useSynth").and_then(|v| v.as_bool()) {
        options.use_synth = use_synth;
    } else if crate::synth_probe::probe_enabled_from_env() {
        options.use_synth = true;
    }

    if mode == "status_only" {
        let graph = load_graph(&root);
        let repo = repository_state(&root, graph.as_ref());
        return ToolEnvelope::ok(
            repo,
            json!({ "indexed": graph.is_some(), "mode": "status_only" }),
            vec![],
            vec![],
            Metrics {
                elapsed_ms: started.elapsed().as_millis() as u64,
                node_count: graph.as_ref().map(|g| g.nodes.len()).unwrap_or(0),
                edge_count: graph.as_ref().map(|g| g.edges.len()).unwrap_or(0),
            },
        );
    }

    let git = read_git_state(&root);
    let inventory = inventory_files(&root, &options);
    let stored_hashes = load_file_hashes(&root);
    let refresh_mode;
    let graph = if mode == "auto" {
        if let (Some(existing), Some(stored)) = (load_graph(&root), stored_hashes.as_ref()) {
            if existing.schema_version == GRAPH_SCHEMA_VERSION
                && stored.schema_version == GRAPH_SCHEMA_VERSION
                && stored.file_hashes == inventory
            {
                refresh_mode = "cache_hit";
                ArchitectureGraph {
                    repository: crate::types::RepositorySnapshot {
                        root: existing.repository.root.clone(),
                        git_commit: git.commit.clone(),
                        worktree_dirty: git.dirty,
                    },
                    ..existing
                }
            } else if existing.schema_version == GRAPH_SCHEMA_VERSION
                && stored.schema_version == GRAPH_SCHEMA_VERSION
            {
                let mut changed = HashSet::new();
                let mut deleted = HashSet::new();
                for (path, hash) in &inventory {
                    match stored.file_hashes.get(path) {
                        Some(previous) if previous == hash => {}
                        Some(_) | None => {
                            changed.insert(path.clone());
                        }
                    }
                }
                for path in stored.file_hashes.keys() {
                    if !inventory.contains_key(path) {
                        deleted.insert(path.clone());
                    }
                }

                let affected = changed.len() + deleted.len();
                if affected > 0 && affected * 2 <= inventory.len().max(1) {
                    refresh_mode = "incremental";
                    incremental_refresh(
                        &root,
                        &options,
                        existing,
                        &changed,
                        &deleted,
                        git.commit.clone(),
                        git.dirty,
                    )
                } else {
                    refresh_mode = "full";
                    scan_repository(&root, &options, git.commit.clone(), git.dirty)
                }
            } else {
                refresh_mode = "full";
                scan_repository(&root, &options, git.commit.clone(), git.dirty)
            }
        } else {
            refresh_mode = "full";
            scan_repository(&root, &options, git.commit.clone(), git.dirty)
        }
    } else {
        refresh_mode = "full";
        scan_repository(&root, &options, git.commit.clone(), git.dirty)
    };

    if refresh_mode != "cache_hit" {
        if let Err(err) = save_graph(&root, &graph) {
            return ToolEnvelope::error(
                "INTERNAL_ERROR",
                &format!("Failed to persist index: {err}"),
                None,
            );
        }
        if let Err(err) = save_file_hashes(
            &root,
            &FileHashManifest {
                schema_version: GRAPH_SCHEMA_VERSION.into(),
                file_hashes: inventory,
            },
        ) {
            return ToolEnvelope::error(
                "INTERNAL_ERROR",
                &format!("Failed to persist file hash manifest: {err}"),
                None,
            );
        }
    }

    let manifest_nodes = graph.nodes.iter().filter(|n| n.kind == "package").count();
    let module_nodes = graph.nodes.iter().filter(|n| n.kind == "module").count();
    let doc_nodes = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "document" || n.kind == "adr")
        .count();
    let coverage_ast = if module_nodes == 0 {
        0.0
    } else {
        (module_nodes as f64 / graph.nodes.len().max(1) as f64).min(1.0)
    };
    let coverage_docs = if doc_nodes == 0 {
        0.0
    } else {
        (doc_nodes as f64 / graph.nodes.len().max(1) as f64).min(1.0)
    };

    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({
            "indexed": true,
            "refreshMode": refresh_mode,
            "filesScanned": graph.nodes.len(),
            "filesIndexed": graph.nodes.len(),
            "nodes": graph.nodes.len(),
            "edges": graph.edges.len(),
            "extractors": graph.extractors,
            "languages": language_surface_stats(&graph),
            "scan": {
                "include": options.include,
                "exclude": options.exclude,
                "maxFileBytes": options.max_file_bytes,
                "useSynth": options.use_synth,
            },
            "coverage": {
                "ast": coverage_ast,
                "manifests": manifest_nodes,
                "docs": coverage_docs,
                "symbols": graph.nodes.iter().filter(|n| n.kind == "symbol").count(),
                "routes": graph.nodes.iter().filter(|n| n.kind == "route").count(),
            }
        }),
        graph.evidence.clone(),
        vec![],
        metrics(started, &graph),
    )
}

fn architecture_status(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let repo = repository_state(&root, Some(&graph));
    let synth_active = graph
        .extractors
        .iter()
        .any(|extractor| extractor.starts_with("synth-"));
    let gaps = coverage_gaps(&graph, synth_active);
    let mut relation_kinds: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    for e in &graph.edges {
        *relation_kinds.entry(e.kind.clone()).or_default() += 1;
    }
    ToolEnvelope::ok(
        repo,
        json!({
            "indexed": true,
            "extractors": graph.extractors,
            "schemaVersion": graph.schema_version,
            "languages": language_surface_stats(&graph),
            "topFanIn": top_fan_in_modules(&graph, 5),
            "topFanOut": top_fan_out_modules(&graph, 5),
            "orphans": orphan_modules(&graph, 8),
            "cycles": find_short_cycles(&graph, 5, 5),
            "defaultExcludes": crate::scanner::default_excludes(),
            "relationKinds": relation_kinds,
            "coverage": {
                "nodes": graph.nodes.len(),
                "edges": graph.edges.len(),
                "claims": graph.claims.len(),
                "evidence": graph.evidence.len(),
                "synthMode": if synth_active { "active" } else { "off" },
                "importGraphRoute": if synth_active { "synth_ast" } else { "regex_fallback" }
            }
        }),
        graph.evidence.clone(),
        gaps,
        metrics(started, &graph),
    )
}

fn architecture_explain(input: serde_json::Value) -> ToolEnvelope {
    let mut envelope = architecture_overview(input);
    if envelope.status == "ok" {
        if let Some(answer) = envelope.answer.as_mut() {
            if let Some(object) = answer.as_object_mut() {
                object.insert(
                    "summary".into(),
                    json!({
                        "headline": "Repository structure with file-level evidence",
                        "nextQuestions": [
                            "Use architecture_search for a focused behavior",
                            "Use architecture_trace for a call or dependency path",
                            "Use architecture_impact before editing a changed path"
                        ]
                    }),
                );
            }
        }
        envelope.payload = envelope.answer.clone();
    }
    envelope
}

fn architecture_overview(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let depth = input.get("depth").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
    let focus = input.get("focus").and_then(|v| v.as_str()).unwrap_or("all");
    // Graphify-class local neighborhood without a 9th tool: overview focus=node/path/label
    let focus_node = if focus != "all" {
        resolve_node_id(&graph, focus)
    } else {
        None
    };
    let neighbors = if let Some(ref id) = focus_node {
        let mut out = Vec::new();
        for edge in graph.edges.iter().filter(|e| e.from == *id || e.to == *id) {
            let other = if edge.from == *id { &edge.to } else { &edge.from };
            let other_node = graph.nodes.iter().find(|n| n.id == *other);
            out.push(json!({
                "edge": edge.kind,
                "direction": if edge.from == *id { "outgoing" } else { "incoming" },
                "node": {
                    "id": other,
                    "kind": other_node.map(|n| n.kind.clone()),
                    "label": other_node.map(|n| n.label.clone()),
                    "path": other_node.and_then(|n| n.path.clone()),
                },
                "evidenceIds": edge.evidence_ids,
            }));
            if out.len() >= depth * 8 {
                break;
            }
        }
        out
    } else {
        vec![]
    };

    let mut package_nodes: Vec<_> = graph.nodes.iter().filter(|n| n.kind == "package").collect();
    package_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let packages: Vec<_> = package_nodes
        .into_iter()
        .take(depth.max(1) * 4)
        .map(|n| json!({ "id": n.id, "label": n.label, "path": n.path }))
        .collect();
    let mut module_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "module" && n.path.is_some())
        .collect();
    module_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let modules: Vec<_> = module_nodes
        .into_iter()
        .take(depth * 4)
        .map(|n| json!({ "id": n.id, "label": n.label, "path": n.path }))
        .collect();
    let docs: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "document" || n.kind == "adr")
        .take(depth * 2)
        .map(|n| json!({ "id": n.id, "kind": n.kind, "path": n.path }))
        .collect();

    let mut overview_edges: Vec<serde_json::Value> = Vec::new();
    if let Some(ref fid) = focus_node {
        for n in &neighbors {
            let direction = n.get("direction").and_then(|v| v.as_str()).unwrap_or("outgoing");
            let other_id = n
                .get("node")
                .and_then(|node| node.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let edge_kind = n.get("edge").and_then(|v| v.as_str()).unwrap_or("rel");
            if other_id.is_empty() {
                continue;
            }
            let (from, to) = if direction == "incoming" {
                (other_id, fid.as_str())
            } else {
                (fid.as_str(), other_id)
            };
            overview_edges.push(json!({
                "from": from,
                "to": to,
                "edge": edge_kind,
                "fromNode": node_summary(&graph, from),
                "toNode": node_summary(&graph, to),
            }));
        }
    }

    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({
            "focus": focus,
            "focusNodeId": focus_node,
            "neighbors": neighbors,
            "packages": packages,
            "modules": modules,
            "documents": docs,
            "counts": {
                "nodes": graph.nodes.len(),
                "edges": graph.edges.len(),
                "evidence": graph.evidence.len(),
                "claims": graph.claims.len(),
                "byKind": {
                    "package": graph.nodes.iter().filter(|n| n.kind == "package").count(),
                    "module": graph.nodes.iter().filter(|n| n.kind == "module").count(),
                    "symbol": graph.nodes.iter().filter(|n| n.kind == "symbol").count(),
                    "route": graph.nodes.iter().filter(|n| n.kind == "route").count(),
                    "schema": graph.nodes.iter().filter(|n| n.kind == "schema").count(),
                    "document": graph.nodes.iter().filter(|n| n.kind == "document" || n.kind == "adr").count(),
                    "workflow": graph.nodes.iter().filter(|n| n.kind == "workflow").count(),
                }
            },
            "extractors": graph.extractors,
            "languages": language_surface_stats(&graph),
            "cycles": find_short_cycles(&graph, 8, 5),
            "topFanIn": top_fan_in_modules(&graph, depth * 4),
            "topFanOut": top_fan_out_modules(&graph, depth * 4),
            "orphans": orphan_modules(&graph, depth * 4),
            "claims": graph.claims.iter().take(depth).map(|c| json!({ "id": c.id, "text": c.text })).collect::<Vec<_>>(),
            "mermaid": if overview_edges.is_empty() {
                serde_json::Value::Null
            } else {
                json!(edges_to_mermaid(&graph, &overview_edges, "overview focus neighborhood"))
            }
        }),
        graph.evidence.clone(),
        coverage_gaps(
            &graph,
            graph
                .extractors
                .iter()
                .any(|extractor| extractor.starts_with("synth-")),
        ),
        metrics(started, &graph),
    )
}

fn architecture_search(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let query = input
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_lowercase();
    let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
    let types: HashSet<String> = input
        .get("types")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    // Empty-query browse mode: rank by structural fan-in (Graphify-class "show me important nodes").
    let mut fan_in: std::collections::HashMap<&str, u64> = std::collections::HashMap::new();
    if query.is_empty() {
        for e in &graph.edges {
            if matches!(e.kind.as_str(), "imports" | "calls" | "depends_on") {
                *fan_in.entry(e.to.as_str()).or_default() += 1;
            }
        }
    }

    let mut scored: Vec<(f64, serde_json::Value)> = Vec::new();
    for node in &graph.nodes {
        if !types.is_empty() && !types.contains(&node.kind) {
            continue;
        }
        let label = node.label.to_lowercase();
        let path = node.path.clone().unwrap_or_default().to_lowercase();
        let kind = node.kind.to_lowercase();
        let hay = format!("{label} {path} {kind}");
        if !query.is_empty() && !hay.contains(&query) {
            continue;
        }
        let (base_score, match_kind) = if query.is_empty() {
            let fi = *fan_in.get(node.id.as_str()).unwrap_or(&0);
            (1.0 + (fi as f64), "browse_fan_in")
        } else if label == query {
            (10.0, "exact_label")
        } else if label.starts_with(&query) {
            (8.0, "label_prefix")
        } else if label.contains(&query) {
            (6.0, "label_substring")
        } else if path.ends_with(&query) || path.contains(&format!("/{query}")) {
            (5.0, "path_suffix_or_segment")
        } else if path.contains(&query) {
            (3.5, "path_substring")
        } else {
            (1.0, "kind_or_weak")
        };
        // Prefer symbols and routes slightly for architecture questions.
        // Empty browse: skip root repository node noise
        if query.is_empty() && node.kind == "repository" {
            continue;
        }
        let kind_boost = match node.kind.as_str() {
            "symbol" => 0.4,
            "route" => 0.3,
            "schema" => 0.25,
            "module" => 0.15,
            "package" => 0.1,
            _ => 0.0,
        };
        let score = base_score + kind_boost;
        let mut score_explain = vec![
            match_kind.to_string(),
            format!("kind={}", node.kind),
            format!("kindBoost={kind_boost}"),
        ];
        if query.is_empty() {
            let fi = *fan_in.get(node.id.as_str()).unwrap_or(&0);
            score_explain.push(format!("fanIn={fi}"));
        }
        scored.push((
            score,
            json!({
                "id": node.id,
                "kind": node.kind,
                "label": node.label,
                "path": node.path,
                "score": score,
                "scoreExplain": score_explain,
            }),
        ));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut matches: Vec<serde_json::Value> = scored.into_iter().take(limit).map(|(_, v)| v).collect();

    let include_neighbors = input
        .get("includeNeighbors")
        .or_else(|| input.get("include_neighbors"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if include_neighbors {
        for m in matches.iter_mut() {
            let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if id.is_empty() {
                continue;
            }
            let mut neighbors = Vec::new();
            for edge in graph.edges.iter().filter(|e| e.from == id || e.to == id) {
                let other = if edge.from == id { &edge.to } else { &edge.from };
                let other_node = graph.nodes.iter().find(|n| n.id == *other);
                neighbors.push(json!({
                    "edge": edge.kind,
                    "direction": if edge.from == id { "outgoing" } else { "incoming" },
                    "id": other,
                    "kind": other_node.map(|n| n.kind.clone()),
                    "label": other_node.map(|n| n.label.clone()),
                    "path": other_node.and_then(|n| n.path.clone()),
                }));
                if neighbors.len() >= 8 {
                    break;
                }
            }
            m.as_object_mut()
                .map(|obj| obj.insert("neighbors".into(), json!(neighbors)));
        }
    }

    let include_evidence = input
        .get("includeEvidence")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let evidence = if include_evidence {
        collect_evidence_for_nodes(&graph, &matches)
    } else {
        vec![]
    };

    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({ "matches": matches, "includeNeighbors": include_neighbors }),
        evidence,
        coverage_gaps(
            &graph,
            graph
                .extractors
                .iter()
                .any(|extractor| extractor.starts_with("synth-")),
        ),
        metrics(started, &graph),
    )
}




/// Build mermaid graph LR from impact/neighborhood edges with from/to labels.
fn edges_to_mermaid(graph: &ArchitectureGraph, edges: &[serde_json::Value], title: &str) -> String {
    let mut lines = vec![format!("%% {title}"), "graph LR".to_string()];
    if edges.is_empty() {
        lines.push("  empty[\"no edges\"]".into());
        return lines.join("\n");
    }
    for (i, edge) in edges.iter().enumerate() {
        let from_id = edge.get("from").and_then(|v| v.as_str()).unwrap_or("?");
        let to_id = edge.get("to").and_then(|v| v.as_str()).unwrap_or("?");
        let kind = edge
            .get("edge")
            .or_else(|| edge.get("kind"))
            .and_then(|v| v.as_str())
            .unwrap_or("rel");
        let from_label = edge
            .get("fromNode")
            .and_then(|n| n.get("label"))
            .and_then(|v| v.as_str())
            .or_else(|| graph.nodes.iter().find(|n| n.id == from_id).map(|n| n.label.as_str()))
            .unwrap_or(from_id);
        let to_label = edge
            .get("toNode")
            .and_then(|n| n.get("label"))
            .and_then(|v| v.as_str())
            .or_else(|| graph.nodes.iter().find(|n| n.id == to_id).map(|n| n.label.as_str()))
            .unwrap_or(to_id);
        let from_s = sanitize_mermaid_id(from_label, i, "f");
        let to_s = sanitize_mermaid_id(to_label, i, "t");
        lines.push(format!(
            "  {from_s}[\"{}\"] -->|{}| {to_s}[\"{}\"]",
            escape_mermaid_label(from_label),
            kind,
            escape_mermaid_label(to_label)
        ));
    }
    lines.join("\n")
}

fn hops_to_mermaid(graph: &ArchitectureGraph, hops: &[serde_json::Value]) -> String {
    let mut lines = vec!["graph LR".to_string()];
    for (i, hop) in hops.iter().enumerate() {
        let from = hop
            .get("fromNode")
            .and_then(|n| n.get("label"))
            .or_else(|| hop.get("from"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let to = hop
            .get("toNode")
            .and_then(|n| n.get("label"))
            .or_else(|| hop.get("to"))
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let edge = hop.get("kind").and_then(|v| v.as_str()).unwrap_or("rel");
        let from_s = sanitize_mermaid_id(from, i, "f");
        let to_s = sanitize_mermaid_id(to, i, "t");
        lines.push(format!(
            "  {from_s}[\"{}\"] -->|{}| {to_s}[\"{}\"]",
            escape_mermaid_label(from),
            edge,
            escape_mermaid_label(to)
        ));
    }
    if hops.is_empty() {
        lines.push("  empty[\"no path\"]".into());
    }
    let _ = graph;
    lines.join("\n")
}

fn sanitize_mermaid_id(label: &str, i: usize, prefix: &str) -> String {
    let cleaned: String = label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{prefix}{i}_{cleaned}")
}

fn escape_mermaid_label(label: &str) -> String {
    label.replace('"', "'")
}

fn architecture_path(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let from = input
        .get("from")
        .or_else(|| input.get("source"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let to = input
        .get("to")
        .or_else(|| input.get("target"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if from.is_empty() || to.is_empty() {
        return ToolEnvelope::error(
            "INVALID_PATH_QUERY",
            "architecture_path requires from/source and to/target.",
            Some("Pass from and to node ids, paths, labels, or path::symbol."),
        );
    }
    let relation = input
        .get("relation")
        .and_then(|v| v.as_str())
        .unwrap_or("any");
    let max_depth = input
        .get("maxDepth")
        .or_else(|| input.get("max_depth"))
        .and_then(|v| v.as_u64())
        .unwrap_or(8) as usize;

    let from_id = resolve_node_id(&graph, from);
    let to_id = resolve_node_id(&graph, to);
    let (node_path, hops) = match (from_id.as_deref(), to_id.as_deref()) {
        (Some(f), Some(t)) => bfs_path_detailed(&graph, f, t, relation, max_depth),
        _ => (vec![], vec![]),
    };

    let mut gaps = Vec::new();
    let mut suggestions = serde_json::Map::new();
    if from_id.is_none() {
        gaps.push(format!("Could not resolve path start: {from}"));
        suggestions.insert("from".into(), json!(suggest_nodes(&graph, from, 5)));
    }
    if to_id.is_none() {
        gaps.push(format!("Could not resolve path end: {to}"));
        suggestions.insert("to".into(), json!(suggest_nodes(&graph, to, 5)));
    }
    if node_path.is_empty() && from_id.is_some() && to_id.is_some() {
        gaps.push("No path found between the requested entities under the relation/depth budget.".into());
    }
    gaps.extend(coverage_gaps(
        &graph,
        graph
            .extractors
            .iter()
            .any(|extractor| extractor.starts_with("synth-")),
    ));

    let hop_count = hops.len();
    let evidence = collect_path_evidence(&graph, &hops);
    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({
            "from": from,
            "to": to,
            "fromId": from_id,
            "toId": to_id,
            "relation": relation,
            "maxDepth": max_depth,
            "hopCount": hop_count,
            "nodes": node_path,
            "hops": hops,
            "mermaid": hops_to_mermaid(&graph, &hops),
            "suggestions": suggestions,
        }),
        evidence,
        gaps,
        metrics(started, &graph),
    )
}

fn architecture_trace(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let from = input
        .get("from")
        .or_else(|| input.get("source"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let to = input
        .get("to")
        .or_else(|| input.get("target"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let relation = input.get("relation").and_then(|v| v.as_str()).unwrap_or("any");
    let max_depth = input
        .get("maxDepth")
        .or_else(|| input.get("max_depth"))
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    let from_id = resolve_node_id(&graph, from);
    let to_id = resolve_node_id(&graph, to);
    let (path, hops) = match (from_id.as_deref(), to_id.as_deref()) {
        (Some(f), Some(t)) => bfs_path_detailed(&graph, f, t, relation, max_depth),
        _ => (vec![], vec![]),
    };

    let mut gaps = Vec::new();
    if from_id.is_none() {
        gaps.push(format!("Could not resolve trace start: {from}"));
    }
    if to_id.is_none() {
        gaps.push(format!("Could not resolve trace end: {to}"));
    }
    if path.is_empty() && from_id.is_some() && to_id.is_some() {
        gaps.push("No trace path found between the requested entities under the relation/depth budget.".into());
    }

    let evidence = if hops.is_empty() {
        graph.evidence.clone()
    } else {
        collect_path_evidence(&graph, &hops)
    };
    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({
            "from": from,
            "to": to,
            "fromId": from_id,
            "toId": to_id,
            "relation": relation,
            "maxDepth": max_depth,
            "hopCount": hops.len(),
            "path": path,
            "hops": hops,
        }),
        evidence,
        gaps,
        metrics(started, &graph),
    )
}

fn architecture_impact(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let use_git_diff = input
        .get("useGitDiff")
        .or_else(|| input.get("use_git_diff"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let git_base = input
        .get("gitBase")
        .or_else(|| input.get("git_base"))
        .and_then(|v| v.as_str());
    let mut changed: Vec<String> = input
        .get("changedPaths")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    // Allow CLI to pass a single sentinel path "--git-diff"
    if changed.iter().any(|p| p == "--git-diff") || use_git_diff {
        changed = list_changed_paths(&root, git_base);
    }

    let max_depth = input
        .get("maxDepth")
        .or_else(|| input.get("max_depth"))
        .and_then(|v| v.as_u64())
        .unwrap_or(2) as usize;

    let mut direct = Vec::new();
    let mut seed_ids = Vec::new();
    let mut unknown = Vec::new();
    for path in &changed {
        let mut found = false;
        for node in graph.nodes.iter().filter(|n| n.path.as_deref() == Some(path.as_str())) {
            found = true;
            direct.push(json!({ "id": node.id, "path": node.path, "kind": node.kind, "label": node.label }));
            seed_ids.push(node.id.clone());
        }
        if !found {
            unknown.push(json!({
                "path": path,
                "reason": "no_indexed_nodes_for_path",
                "hint": "Path may be unindexed, excluded, binary, or outside extractors.",
            }));
        }
    }

    // Outgoing: dependencies of changed nodes. Incoming: reverse dependents (blast radius).
    let mut outgoing = Vec::new();
    let mut incoming = Vec::new();
    let mut seen_out = std::collections::HashSet::<String>::new();
    let mut seen_in = std::collections::HashSet::<String>::new();
    for id in &seed_ids {
        for edge in graph.edges.iter().filter(|e| e.from == *id) {
            let key = format!("{}->{}:{}", edge.from, edge.to, edge.kind);
            if seen_out.insert(key) {
                outgoing.push(json!({
                    "edge": edge.kind,
                    "from": edge.from,
                    "to": edge.to,
                    "direction": "outgoing",
                    "depth": 1
                }));
            }
        }
        for edge in graph.edges.iter().filter(|e| e.to == *id) {
            let key = format!("{}->{}:{}", edge.from, edge.to, edge.kind);
            if seen_in.insert(key) {
                incoming.push(json!({
                    "edge": edge.kind,
                    "from": edge.from,
                    "to": edge.to,
                    "direction": "incoming",
                    "depth": 1
                }));
            }
        }
    }

    // Multi-hop expansion (bounded) for reverse dependents and forward deps.
    if max_depth > 1 {
        // Forward BFS from seeds
        let mut frontier = seed_ids.clone();
        for depth in 2..=max_depth {
            let mut next = Vec::new();
            for id in &frontier {
                for edge in graph.edges.iter().filter(|e| e.from == *id) {
                    let key = format!("{}->{}:{}", edge.from, edge.to, edge.kind);
                    if seen_out.insert(key) {
                        outgoing.push(json!({
                            "edge": edge.kind,
                            "from": edge.from,
                            "to": edge.to,
                            "direction": "outgoing",
                            "depth": depth
                        }));
                        next.push(edge.to.clone());
                    }
                }
            }
            frontier = next;
        }
        // Reverse BFS (who depends on seeds)
        frontier = seed_ids.clone();
        for depth in 2..=max_depth {
            let mut next = Vec::new();
            for id in &frontier {
                for edge in graph.edges.iter().filter(|e| e.to == *id) {
                    let key = format!("{}->{}:{}", edge.from, edge.to, edge.kind);
                    if seen_in.insert(key) {
                        incoming.push(json!({
                            "edge": edge.kind,
                            "from": edge.from,
                            "to": edge.to,
                            "direction": "incoming",
                            "depth": depth
                        }));
                        next.push(edge.from.clone());
                    }
                }
            }
            frontier = next;
        }
    }

    // Attach node summaries for agent readability
    let enrich = |v: &mut serde_json::Value| {
        if let Some(obj) = v.as_object_mut() {
            if let Some(from) = obj.get("from").and_then(|x| x.as_str()).map(str::to_string) {
                obj.insert("fromNode".into(), node_summary(&graph, &from));
            }
            if let Some(to) = obj.get("to").and_then(|x| x.as_str()).map(str::to_string) {
                obj.insert("toNode".into(), node_summary(&graph, &to));
            }
        }
    };
    for e in &mut outgoing {
        enrich(e);
    }
    for e in &mut incoming {
        enrich(e);
    }

    // Deterministic edge ordering for stable agent answers / golden parity
    let edge_key = |v: &serde_json::Value| -> String {
        format!(
            "{}:{}:{}:{}",
            v.get("depth").and_then(|x| x.as_u64()).unwrap_or(0),
            v.get("edge").and_then(|x| x.as_str()).unwrap_or(""),
            v.get("from").and_then(|x| x.as_str()).unwrap_or(""),
            v.get("to").and_then(|x| x.as_str()).unwrap_or(""),
        )
    };
    outgoing.sort_by(|a, b| edge_key(a).cmp(&edge_key(b)));
    incoming.sort_by(|a, b| edge_key(a).cmp(&edge_key(b)));
    let transitive = outgoing.clone();

    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({
            "changedPaths": changed,
            "changedPathSource": if use_git_diff || input
                .get("changedPaths")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|x| x.as_str() == Some("--git-diff")))
                .unwrap_or(false)
            {
                "git"
            } else {
                "explicit"
            },
            "maxDepth": max_depth,
            "directImpact": direct,
            "outgoingImpact": outgoing,
            "incomingImpact": incoming,
            "transitiveImpact": transitive,
            "unknownImpact": unknown,
            "mermaid": {
                "incoming": edges_to_mermaid(&graph, &incoming, "incoming blast radius"),
                "outgoing": edges_to_mermaid(&graph, &outgoing, "outgoing dependencies"),
            }
        }),
        graph.evidence.clone(),
        coverage_gaps(
            &graph,
            graph
                .extractors
                .iter()
                .any(|extractor| extractor.starts_with("synth-")),
        ),
        metrics(started, &graph),
    )
}

fn architecture_evidence(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let ids: Vec<String> = input
        .get("ids")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let mut found = Vec::new();
    let mut resolved = Vec::new();
    let mut missing = Vec::new();
    let mut seen_ev = std::collections::HashSet::<String>::new();

    for id in &ids {
        let mut got = false;
        if let Some(ev) = graph.evidence.iter().find(|e| e.id == *id) {
            if seen_ev.insert(ev.id.clone()) {
                found.push(ev.clone());
            }
            got = true;
        }
        if let Some(node) = graph.nodes.iter().find(|n| n.id == *id) {
            for ev_id in &node.evidence_ids {
                if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                    if seen_ev.insert(ev.id.clone()) {
                        found.push(ev.clone());
                    }
                    got = true;
                }
            }
            resolved.push(json!({ "id": id, "kind": "node", "node": node_summary(&graph, id) }));
        }
        if let Some(edge) = graph.edges.iter().find(|e| e.id == *id) {
            for ev_id in &edge.evidence_ids {
                if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                    if seen_ev.insert(ev.id.clone()) {
                        found.push(ev.clone());
                    }
                    got = true;
                }
            }
            resolved.push(json!({
                "id": id,
                "kind": "edge",
                "edge": edge.kind,
                "from": edge.from,
                "to": edge.to,
                "fromNode": node_summary(&graph, &edge.from),
                "toNode": node_summary(&graph, &edge.to),
            }));
        }
        // label/path fallback
        if !got {
            if let Some(nid) = resolve_node_id(&graph, id) {
                if let Some(node) = graph.nodes.iter().find(|n| n.id == nid) {
                    for ev_id in &node.evidence_ids {
                        if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                            if seen_ev.insert(ev.id.clone()) {
                                found.push(ev.clone());
                            }
                            got = true;
                        }
                    }
                    resolved.push(json!({ "id": id, "kind": "resolved_node", "node": node_summary(&graph, &nid) }));
                }
            }
        }
        if !got {
            missing.push(id.clone());
        }
    }

    if found.is_empty() && resolved.is_empty() {
        return ToolEnvelope::error(
            "EVIDENCE_NOT_FOUND",
            "No evidence found for the requested ids.",
            Some("Call architecture_search, architecture_path, or architecture_overview first."),
        );
    }

    let mut gaps = Vec::new();
    if !missing.is_empty() {
        gaps.push(format!("No evidence for ids: {}", missing.join(", ")));
    }

    let repo = repository_state(&root, Some(&graph));
    ToolEnvelope::ok(
        repo,
        json!({
            "items": found.len(),
            "ids": ids,
            "resolved": resolved,
            "missing": missing,
        }),
        found,
        gaps,
        metrics(started, &graph),
    )
}

/// Advanced: pack a focused neighborhood for agent context windows (Graphify-class).
/// Does not replace primary tools; composes focus + neighbors + evidence + local fan.
fn architecture_context_pack(input: serde_json::Value) -> ToolEnvelope {
    let started = Instant::now();
    let root = match resolve_root(&input) {
        Ok(r) => r,
        Err(e) => return e,
    };
    let graph = match require_graph(&root) {
        Ok(g) => g,
        Err(e) => return e,
    };
    let focus = input
        .get("focus")
        .or_else(|| input.get("path"))
        .or_else(|| input.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if focus.is_empty() {
        return ToolEnvelope::error(
            "INVALID_INPUT",
            "architecture_context_pack requires focus (path, label, or node id)",
            Some("pass focus as path, label, or node id"),
        );
    }
    let max_neighbors = input
        .get("maxNeighbors")
        .or_else(|| input.get("max_neighbors"))
        .and_then(|v| v.as_u64())
        .unwrap_or(12) as usize;
    let max_evidence = input
        .get("maxEvidence")
        .or_else(|| input.get("max_evidence"))
        .and_then(|v| v.as_u64())
        .unwrap_or(8) as usize;

    let focus_id = match resolve_node_id(&graph, focus) {
        Some(id) => id,
        None => {
            let suggestions = suggest_nodes(&graph, focus, 8);
            let mut env = ToolEnvelope::error(
                "NOT_FOUND",
                &format!("No node matched focus={focus}"),
                Some("retry with architecture_search or a path from suggestions"),
            );
            env.repository = Some(repository_state(&root, Some(&graph)));
            env.answer = Some(json!({
                "focus": focus,
                "suggestions": suggestions,
            }));
            return env;
        }
    };

    let focus_node = graph.nodes.iter().find(|n| n.id == focus_id);
    let mut neighbors = Vec::new();
    for edge in graph.edges.iter().filter(|e| e.from == focus_id || e.to == focus_id) {
        let other = if edge.from == focus_id { &edge.to } else { &edge.from };
        neighbors.push(json!({
            "edge": edge.kind,
            "direction": if edge.from == focus_id { "outgoing" } else { "incoming" },
            "node": node_summary(&graph, other),
            "evidenceIds": edge.evidence_ids,
        }));
        if neighbors.len() >= max_neighbors {
            break;
        }
    }

    let mut evidence = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    if let Some(node) = focus_node {
        for ev_id in &node.evidence_ids {
            if evidence.len() >= max_evidence {
                break;
            }
            if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                if seen.insert(ev.id.clone()) {
                    evidence.push(ev.clone());
                }
            }
        }
    }
    for n in &neighbors {
        if evidence.len() >= max_evidence {
            break;
        }
        if let Some(ids) = n.get("evidenceIds").and_then(|v| v.as_array()) {
            for idv in ids {
                if evidence.len() >= max_evidence {
                    break;
                }
                if let Some(id) = idv.as_str() {
                    if let Some(ev) = graph.evidence.iter().find(|e| e.id == id) {
                        if seen.insert(ev.id.clone()) {
                            evidence.push(ev.clone());
                        }
                    }
                }
            }
        }
    }

    // Same-directory modules when focus has a path (local composition without a 2nd tool call).
    let mut co_located = Vec::new();
    if let Some(path) = focus_node.and_then(|n| n.path.as_ref()) {
        let parent = std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if !parent.is_empty() {
            for n in &graph.nodes {
                if n.id == focus_id {
                    continue;
                }
                if let Some(ref p) = n.path {
                    if std::path::Path::new(p)
                        .parent()
                        .map(|x| x.to_string_lossy() == parent)
                        .unwrap_or(false)
                    {
                        co_located.push(node_summary(&graph, &n.id));
                    }
                }
                if co_located.len() >= max_neighbors {
                    break;
                }
            }
        }
    }

    let repo = repository_state(&root, Some(&graph));
    let mut warnings = coverage_gaps(
        &graph,
        graph
            .extractors
            .iter()
            .any(|extractor| extractor.starts_with("synth-")),
    );
    warnings.push(
        "architecture_context_pack is advanced: prefer primary tools when a single question is enough"
            .into(),
    );

    let mut neighbor_edges: Vec<serde_json::Value> = Vec::new();
    for n in &neighbors {
        let direction = n.get("direction").and_then(|v| v.as_str()).unwrap_or("outgoing");
        let other_id = n
            .get("node")
            .and_then(|node| node.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let edge_kind = n.get("edge").and_then(|v| v.as_str()).unwrap_or("rel");
        if other_id.is_empty() {
            continue;
        }
        let (from, to) = if direction == "incoming" {
            (other_id, focus_id.as_str())
        } else {
            (focus_id.as_str(), other_id)
        };
        neighbor_edges.push(json!({
            "from": from,
            "to": to,
            "edge": edge_kind,
            "fromNode": node_summary(&graph, from),
            "toNode": node_summary(&graph, to),
        }));
    }

    ToolEnvelope::ok(
        repo,
        json!({
            "focus": focus,
            "focusNode": node_summary(&graph, &focus_id),
            "neighbors": neighbors,
            "coLocated": co_located,
            "evidence": evidence,
            "localFan": {
                "topFanIn": top_fan_in_modules(&graph, 5),
                "topFanOut": top_fan_out_modules(&graph, 5),
            },
            "counts": {
                "neighbors": neighbors.len(),
                "coLocated": co_located.len(),
                "evidence": evidence.len(),
            },
            "mermaid": edges_to_mermaid(&graph, &neighbor_edges, "context pack neighborhood"),
        }),
        evidence.clone(),
        warnings,
        metrics(started, &graph),
    )
}


fn coverage_gaps(graph: &ArchitectureGraph, synth_active: bool) -> Vec<String> {
    let mut gaps = Vec::new();
    // Honest residual gaps only — routes/schemas ARE implemented when present in graph.
    if graph.nodes.iter().all(|n| n.kind != "route") {
        gaps.push("No HTTP route nodes found in this index (TS Express-style extractors may not match this repo).".into());
    }
    if graph.nodes.iter().all(|n| n.kind != "schema") {
        gaps.push("No schema nodes found in this index (zod/JSON-schema extractors may not match this repo).".into());
    }
    if graph.nodes.iter().all(|n| n.kind != "symbol") {
        gaps.push("No symbol nodes found — language extractors may not cover this repository yet.".into());
    }
    let langs = language_module_counts(graph);
    if langs.get("rust").copied().unwrap_or(0) == 0
        && langs.get("go").copied().unwrap_or(0) == 0
        && langs.get("python").copied().unwrap_or(0) == 0
        && langs.get("typescript").copied().unwrap_or(0) == 0
    {
        gaps.push("No recognized source-language modules indexed (TS/JS/Python/Rust/Go).".into());
    }
    if graph.repository.worktree_dirty {
        gaps.push("Working tree has uncommitted changes.".into());
    }
    if !synth_active {
        gaps.push(
            "Synth AST substrate is off by default (importGraphRoute=regex_fallback). Set ARCHITECTURE_READER_USE_SYNTH=1 to enable synth_ast in CI or local runs.".into(),
        );
    }
    gaps
}

fn language_module_counts(graph: &ArchitectureGraph) -> std::collections::BTreeMap<String, u64> {
    let mut counts = std::collections::BTreeMap::<String, u64>::new();
    for node in graph.nodes.iter().filter(|n| n.kind == "module") {
        let path = node.path.as_deref().unwrap_or("");
        let lang = if path.ends_with(".ts")
            || path.ends_with(".tsx")
            || path.ends_with(".js")
            || path.ends_with(".jsx")
            || path.ends_with(".mjs")
            || path.ends_with(".cjs")
        {
            "typescript"
        } else if path.ends_with(".py") {
            "python"
        } else if path.ends_with(".rs") {
            "rust"
        } else if path.ends_with(".go") {
            "go"
        } else if path.ends_with(".java") {
            "java"
        } else if path.ends_with(".cs") {
            "csharp"
        } else if path.ends_with(".kt") || path.ends_with(".kts") {
            "kotlin"
        } else if path.ends_with(".rb") {
            "ruby"
        } else if path.ends_with(".php") {
            "php"
        } else if path.ends_with(".c")
            || path.ends_with(".h")
            || path.ends_with(".cc")
            || path.ends_with(".cpp")
            || path.ends_with(".cxx")
            || path.ends_with(".hpp")
            || path.ends_with(".hh")
        {
            "c"
        } else if path.ends_with(".sh")
            || path.ends_with(".bash")
            || path.ends_with(".zsh")
            || path.ends_with(".ksh")
        {
            "shell"
        } else if path.ends_with(".sql") {
            "sql"
        } else if path.is_empty() {
            "external"
        } else {
            "other"
        };
        *counts.entry(lang.to_string()).or_default() += 1;
    }
    counts
}

fn language_surface_stats(graph: &ArchitectureGraph) -> serde_json::Map<String, serde_json::Value> {
    language_module_counts(graph)
        .into_iter()
        .map(|(k, v)| (k, json!(v)))
        .collect()
}

fn resolve_node_id(graph: &ArchitectureGraph, needle: &str) -> Option<String> {
    if let Some(node) = graph.nodes.iter().find(|n| n.id == needle) {
        return Some(node.id.clone());
    }

    if let Some(node) = graph.nodes.iter().find(|n| n.path.as_deref() == Some(needle)) {
        return Some(node.id.clone());
    }

    if let Some((path, symbol)) = needle.split_once("::") {
        let qualified_matches: Vec<_> = graph
            .nodes
            .iter()
            .filter(|n| n.kind == "symbol" && n.label == symbol && n.path.as_deref() == Some(path))
            .collect();
        if !qualified_matches.is_empty() {
            return Some(prefer_symbol_node_id(&qualified_matches));
        }
    }

    if let Some(node) = graph
        .nodes
        .iter()
        .find(|n| n.kind != "symbol" && n.label == needle)
    {
        return Some(node.id.clone());
    }

    let symbol_matches: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == "symbol" && n.label == needle)
        .collect();
    if symbol_matches.is_empty() {
        return None;
    }

    Some(prefer_symbol_node_id(&symbol_matches))
}

fn prefer_symbol_node_id(nodes: &[&GraphNode]) -> String {
    nodes
        .iter()
        .find(|node| node.id.contains(":export:"))
        .or_else(|| nodes.iter().find(|node| node.id.contains(":function:")))
        .map(|node| node.id.clone())
        .unwrap_or_else(|| nodes[0].id.clone())
}

#[allow(dead_code)]
fn bfs_path(
    graph: &ArchitectureGraph,
    from: &str,
    to: &str,
    relation: &str,
    max_depth: usize,
) -> Vec<String> {
    let adjacency: HashMap<&str, Vec<&str>> = graph
        .edges
        .iter()
        .filter(|e| relation == "any" || e.kind == relation)
        .fold(HashMap::new(), |mut acc, edge| {
            acc.entry(edge.from.as_str()).or_default().push(edge.to.as_str());
            acc
        });

    let mut queue = VecDeque::from([(from.to_string(), vec![from.to_string()])]);
    let mut visited = HashSet::from([from.to_string()]);

    while let Some((current, path)) = queue.pop_front() {
        if path.len() > max_depth {
            continue;
        }
        if current == to {
            return path;
        }
        if let Some(neighbors) = adjacency.get(current.as_str()) {
            for next in neighbors {
                if visited.insert((*next).to_string()) {
                    let mut next_path = path.clone();
                    next_path.push((*next).to_string());
                    queue.push_back(((*next).to_string(), next_path));
                }
            }
        }
    }
    vec![]
}


fn bfs_path_detailed(
    graph: &ArchitectureGraph,
    from: &str,
    to: &str,
    relation: &str,
    max_depth: usize,
) -> (Vec<String>, Vec<serde_json::Value>) {
    // adjacency: from -> list of (to, edge)
    let mut adjacency: HashMap<&str, Vec<(&str, &crate::types::GraphEdge)>> = HashMap::new();
    for edge in &graph.edges {
        if relation != "any" && edge.kind != relation {
            continue;
        }
        adjacency
            .entry(edge.from.as_str())
            .or_default()
            .push((edge.to.as_str(), edge));
    }

    let mut queue: VecDeque<(String, Vec<String>, Vec<String>)> =
        VecDeque::from([(from.to_string(), vec![from.to_string()], vec![])]);
    let mut visited = HashSet::from([from.to_string()]);

    while let Some((current, node_path, edge_ids)) = queue.pop_front() {
        if node_path.len().saturating_sub(1) > max_depth {
            continue;
        }
        if current == to {
            let hops = edge_ids
                .iter()
                .filter_map(|eid| graph.edges.iter().find(|e| &e.id == eid))
                .map(|edge| {
                    let provenance = edge_provenance(graph, edge);
                    json!({
                        "from": edge.from,
                        "to": edge.to,
                        "fromNode": node_summary(graph, &edge.from),
                        "toNode": node_summary(graph, &edge.to),
                        "edgeId": edge.id,
                        "edgeKind": edge.kind,
                        "provenance": provenance,
                    })
                })
                .collect::<Vec<_>>();
            return (node_path, hops);
        }
        if let Some(neighbors) = adjacency.get(current.as_str()) {
            for (next, edge) in neighbors {
                if visited.insert((*next).to_string()) {
                    let mut next_nodes = node_path.clone();
                    next_nodes.push((*next).to_string());
                    let mut next_edges = edge_ids.clone();
                    next_edges.push(edge.id.clone());
                    queue.push_back(((*next).to_string(), next_nodes, next_edges));
                }
            }
        }
    }
    (vec![], vec![])
}

fn edge_provenance(graph: &ArchitectureGraph, edge: &crate::types::GraphEdge) -> &'static str {
    // Graphify-style honesty: extracted = deterministic evidence present; else inferred.
    for ev_id in &edge.evidence_ids {
        if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
            if matches!(ev.confidence, Confidence::Deterministic | Confidence::Derived) {
                return "extracted";
            }
        }
    }
    if edge.evidence_ids.is_empty() {
        return "inferred";
    }
    "inferred"
}

fn collect_path_evidence(
    graph: &ArchitectureGraph,
    hops: &[serde_json::Value],
) -> Vec<EvidenceRef> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for hop in hops {
        let Some(edge_id) = hop.get("edgeId").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(edge) = graph.edges.iter().find(|e| e.id == edge_id) else {
            continue;
        };
        for ev_id in &edge.evidence_ids {
            if !seen.insert(ev_id.clone()) {
                continue;
            }
            if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                out.push(ev.clone());
            }
        }
        for endpoint in [&edge.from, &edge.to] {
            if let Some(node) = graph.nodes.iter().find(|n| &n.id == endpoint) {
                for ev_id in &node.evidence_ids {
                    if !seen.insert(ev_id.clone()) {
                        continue;
                    }
                    if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                        out.push(ev.clone());
                    }
                }
            }
        }
    }
    out
}

fn collect_evidence_for_nodes(graph: &ArchitectureGraph, matches: &[serde_json::Value]) -> Vec<EvidenceRef> {
    let mut out = Vec::new();
    for m in matches {
        let Some(id) = m.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        if let Some(node) = graph.nodes.iter().find(|n| n.id == id) {
            for ev_id in &node.evidence_ids {
                if let Some(ev) = graph.evidence.iter().find(|e| e.id == *ev_id) {
                    out.push(ev.clone());
                }
            }
        }
    }
    out
}