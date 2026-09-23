//! MCP tool parity: the in-process rmcp adapter must match direct architecture-reader-cli envelopes, proving the subprocess bridge removal is faithful.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use architecture_reader_mcp_server::cli_bridge;
use serde_json::{json, Value};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn resolve_cli_binary() -> PathBuf {
    for relative in [
        "target/release/architecture-reader-cli",
        "target/debug/architecture-reader-cli",
    ] {
        let candidate = repo_root().join(relative);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("architecture-reader-cli is not built; run `cargo build --release -p architecture-reader-cli`");
}

fn fixture_root() -> PathBuf {
    repo_root().join("fixtures/sample-repo")
}

fn invoke_cli_direct(tool: &str, input: Value) -> Value {
    let cli = resolve_cli_binary();
    let payload = serde_json::to_string(&json!({ "tool": tool, "input": input }))
        .expect("serialize cli request");

    let mut child = Command::new(cli)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn architecture-reader-cli");

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(payload.as_bytes())
            .expect("write cli request");
    }

    let output = child
        .wait_with_output()
        .expect("wait for architecture-reader-cli");
    assert!(
        output.status.success(),
        "architecture-reader-cli failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("parse cli stdout")
}

fn normalize_envelope(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("tool");
        object.remove("engine");
        if let Some(metrics) = object.get_mut("metrics").and_then(Value::as_object_mut) {
            metrics.remove("elapsedMs");
        }
    }
    value
}

fn assert_parity(id: &str, tool: &str, input: Value) {
    // SAFETY: test-only single-threaded env mutation.
    unsafe {
        std::env::set_var("ARCHITECTURE_READER_CLI", resolve_cli_binary());
    }

    let direct = invoke_cli_direct(tool, input.clone());
    assert_eq!(
        direct.get("status").and_then(Value::as_str),
        Some("ok"),
        "{id}: direct CLI expected ok status"
    );

    let mcp = cli_bridge::invoke_cli_tool(tool, input)
        .unwrap_or_else(|error| panic!("{id}: cli_bridge failed: {error:?}"));
    let structured = mcp
        .structured_content
        .expect("{id}: structured_content should be present");

    assert_eq!(
        normalize_envelope(structured),
        normalize_envelope(direct),
        "{id}: rmcp cli_bridge must match direct CLI envelope"
    );
}

#[test]
fn primary_tools_match_direct_cli_on_fixture_repo() {
    let root = fixture_root().to_string_lossy().to_string();

    // Warm the index once so downstream tools share the same graph state.
    assert_parity(
        "architecture_index_full",
        "architecture_index",
        json!({ "root": root, "mode": "full" }),
    );

    assert_parity(
        "architecture_status",
        "architecture_status",
        json!({ "root": root }),
    );

    assert_parity(
        "architecture_overview",
        "architecture_overview",
        json!({ "root": root, "depth": 2 }),
    );

    let search = invoke_cli_direct(
        "architecture_search",
        json!({ "root": root, "query": "auth", "limit": 5 }),
    );
    assert_eq!(
        search.get("status").and_then(Value::as_str),
        Some("ok"),
        "architecture_search: direct CLI expected ok status"
    );
    assert_parity(
        "architecture_search",
        "architecture_search",
        json!({ "root": root, "query": "auth", "limit": 5 }),
    );

    assert_parity(
        "architecture_trace_calls",
        "architecture_trace",
        json!({
            "root": root,
            "from": "authMiddleware",
            "to": "validateToken",
            "relation": "calls",
            "maxDepth": 4
        }),
    );

    assert_parity(
        "architecture_impact",
        "architecture_impact",
        json!({ "root": root, "changedPaths": ["src/server/routes.ts"] }),
    );

    let evidence_id = search
        .pointer("/answer/matches/0/id")
        .and_then(Value::as_str)
        .or_else(|| {
            search
                .get("evidence")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("id"))
                .and_then(Value::as_str)
        })
        .expect("architecture_search should expose an evidence id for parity");

    assert_parity(
        "architecture_evidence",
        "architecture_evidence",
        json!({ "root": root, "ids": [evidence_id] }),
    );
}