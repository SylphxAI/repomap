use serde::{Deserialize, Serialize};

pub const GRAPH_SCHEMA_VERSION: &str = "0.1.0";
pub const ENGINE_NAME: &str = "architecture-reader-core";
pub const ENGINE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
    Dirty,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Deterministic,
    Derived,
    Inferred,
    Conflicting,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryState {
    pub root: String,
    pub indexed_commit: Option<String>,
    pub current_commit: Option<String>,
    pub freshness: Freshness,
    pub worktree_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub id: String,
    pub kind: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    pub extractor: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub path: Option<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphClaim {
    pub id: String,
    pub text: String,
    pub confidence: Confidence,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default)]
    pub edge_ids: Vec<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchitectureGraph {
    pub schema_version: String,
    pub repository: RepositorySnapshot,
    pub extractors: Vec<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub claims: Vec<GraphClaim>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositorySnapshot {
    pub root: String,
    pub git_commit: Option<String>,
    pub worktree_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub elapsed_ms: u64,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEnvelope {
    /// Family evidence envelope v1.
    #[serde(default = "default_envelope_version")]
    pub envelope_version: String,
    #[serde(default = "default_product")]
    pub product: String,
    #[serde(default = "default_product_version")]
    pub product_version: String,
    #[serde(default = "default_family_route")]
    pub route: serde_json::Value,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<RepositoryState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<serde_json::Value>,
    /// Family payload alias for `answer` when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub gaps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Metrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
}

fn default_envelope_version() -> String { "1".into() }
fn default_product() -> String { "spine".into() }
fn default_product_version() -> String {
    // The published product version, not this crate's 0.1.0. build.rs reads
    // packages/mcp-server/package.json unless SPINE_PRODUCT_VERSION is set.
    option_env!("SPINE_PRODUCT_VERSION")
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
}
fn default_family_route() -> serde_json::Value {
    serde_json::json!({ "engine": "rust-core", "path": "architecture-graph" })
}

impl ToolEnvelope {
    pub fn ok(
        repository: RepositoryState,
        answer: serde_json::Value,
        evidence: Vec<EvidenceRef>,
        gaps: Vec<String>,
        metrics: Metrics,
    ) -> Self {
        Self {
            envelope_version: default_envelope_version(),
            product: default_product(),
            product_version: default_product_version(),
            route: default_family_route(),
            warnings: vec![],
            status: "ok".into(),
            tool: None,
            repository: Some(repository),
            answer: Some(answer.clone()),
            payload: Some(answer),
            evidence,
            gaps,
            metrics: Some(metrics),
            code: None,
            message: None,
            next_action: None,
        }
    }

    pub fn error(code: &str, message: &str, next_action: Option<&str>) -> Self {
        Self {
            envelope_version: default_envelope_version(),
            product: default_product(),
            product_version: default_product_version(),
            route: default_family_route(),
            warnings: vec![],
            status: "error".into(),
            tool: None,
            repository: None,
            answer: None,
            payload: None,
            evidence: vec![],
            gaps: vec![],
            metrics: None,
            code: Some(code.into()),
            message: Some(message.into()),
            next_action: next_action.map(str::to_string),
        }
    }
}
