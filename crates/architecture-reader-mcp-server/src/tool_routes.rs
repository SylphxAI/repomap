//! Explicit shipped routing table for Spine tools.
//! Progressive disclosure: PRIMARY_TOOLS are the agent default path.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRoute {
    RustCore,
}

pub fn route_for_tool(tool: &str) -> Option<ToolRoute> {
    match tool {
        "architecture_index"
        | "architecture_status"
        | "architecture_overview" | "architecture_explain"
        | "architecture_search"
        | "architecture_path"
        | "architecture_trace"
        | "architecture_impact"
        | "architecture_evidence"
        | "architecture_context_pack" => Some(ToolRoute::RustCore),
        _ => None,
    }
}

pub fn is_rust_core_tool(tool: &str) -> bool {
    matches!(route_for_tool(tool), Some(ToolRoute::RustCore))
}

/// Agent default path — keep small and obvious.
pub const PRIMARY_TOOLS: [&str; 7] = [
    "architecture_index",
    "architecture_status",
    "architecture_overview",
    "architecture_explain",
    "architecture_search",
    "architecture_path",
    "architecture_impact",
];

/// Advanced tools (use when primary path is insufficient).
pub const ADVANCED_TOOLS: [&str; 3] = [
    "architecture_trace",
    "architecture_evidence",
    "architecture_context_pack",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_all_primary_tools_to_rust_core() {
        for tool in PRIMARY_TOOLS {
            assert_eq!(route_for_tool(tool), Some(ToolRoute::RustCore));
        }
        for tool in ADVANCED_TOOLS {
            assert_eq!(route_for_tool(tool), Some(ToolRoute::RustCore));
        }
        assert_eq!(route_for_tool("not_a_tool"), None);
    }
}
