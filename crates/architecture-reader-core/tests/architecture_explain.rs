use architecture_reader_core::handle_tool;
use serde_json::json;

#[test]
fn explains_an_indexed_repository_with_next_questions() {
    let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/sample-repo");
    assert_eq!(handle_tool("architecture_index", json!({ "root": fixture, "mode": "full" })).status, "ok");
    let explained = handle_tool("architecture_explain", json!({ "root": fixture }));
    assert_eq!(explained.status, "ok");
    let answer = explained.answer.expect("answer");
    assert!(answer.get("summary").is_some());
    assert!(answer.get("packages").is_some());
}
