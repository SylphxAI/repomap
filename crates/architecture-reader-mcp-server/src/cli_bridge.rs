use rmcp::model::CallToolResult;
use serde_json::Value;

/// Invoke an architecture tool through the in-process Rust engine.
///
/// The engine lives in `architecture-reader-core`, which this server already
/// links. Delegating over a subprocess only added a failure mode — a missing
/// `architecture-reader-cli` binary, which the published native packages never
/// shipped, so every tool failed for an npm user — plus a process spawn per
/// call. Calling `handle_tool` directly keeps the package self-contained and
/// makes every tool call faster.
pub fn invoke_cli_tool(tool: &str, arguments: Value) -> Result<CallToolResult, rmcp::ErrorData> {
    let envelope = architecture_reader_core::handle_tool(tool, arguments);
    let mut structured = serde_json::to_value(&envelope).map_err(|error| {
        rmcp::ErrorData::internal_error(format!("Failed to serialize envelope: {error}"), None)
    })?;

    if structured.get("status").and_then(Value::as_str) != Some("ok") {
        let message = structured
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| structured.get("code").and_then(Value::as_str))
            .unwrap_or("Architecture tool returned an error envelope");
        return Err(rmcp::ErrorData::internal_error(message.to_string(), None));
    }

    if let Some(object) = structured.as_object_mut() {
        object.insert("tool".to_string(), Value::String(tool.to_string()));
        object.insert(
            "engine".to_string(),
            Value::String(architecture_reader_core::ENGINE_NAME.to_string()),
        );
    }

    Ok(CallToolResult::structured(structured))
}
