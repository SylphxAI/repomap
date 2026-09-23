pub mod cli_bridge;
pub mod tool_routes;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Free-form MCP tool args object (root type=object required by rmcp ≥1.8 schema gate).
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
struct FreeformToolArgs(Map<String, Value>);

impl FreeformToolArgs {
    fn into_value(self) -> Value {
        Value::Object(self.0)
    }
}

pub const SERVER_NAME: &str = "spine";
/// The product version, injected at build time from package.json (see the
/// `build:rust` script). Falls back to the release this source last shipped so a
/// plain `cargo build` still compiles.
pub const SERVER_VERSION: &str = match option_env!("SPINE_PRODUCT_VERSION") {
    Some(version) => version,
    None => "0.4.3",
};
pub const SERVER_INSTRUCTIONS: &str =
    "Architecture Reader MCP server (Rust rmcp transport). Index, search, path, trace, impact, and evidence tools run through the Rust evidence-graph engine.";

#[derive(Clone)]
pub struct ArchitectureReaderMcp {
    pub tool_router: ToolRouter<Self>,
}

impl ArchitectureReaderMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    fn invoke(&self, tool: &str, args: Value) -> Result<rmcp::model::CallToolResult, ErrorData> {
        cli_bridge::invoke_cli_tool(tool, args)
    }
}

#[tool_router]
impl ArchitectureReaderMcp {
    #[tool(description = "Index or refresh the architecture evidence graph for a repository.")]
    fn architecture_index(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_index", args.into_value())
    }

    #[tool(description = "Report indexed repository status, extractors, and coverage gaps.")]
    fn architecture_status(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_status", args.into_value())
    }

    #[tool(description = "Return package and module overview slices for the indexed graph.")]
    fn architecture_overview(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_overview", args.into_value())
    }

    #[tool(description = "Explain the repository map with the most useful next architecture questions.")]
    fn architecture_explain(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_explain", args.into_value())
    }

    #[tool(description = "Search the architecture graph with evidence locators.")]
    fn architecture_search(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_search", args.into_value())
    }

    #[tool(description = "Shortest architecture path between two entities with hop provenance (extracted|inferred).")]
    fn architecture_path(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_path", args.into_value())
    }

    #[tool(description = "Trace relations between architecture nodes or symbols.")]
    fn architecture_trace(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_trace", args.into_value())
    }

    #[tool(description = "Compute impact for changed paths in the indexed graph.")]
    fn architecture_impact(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_impact", args.into_value())
    }

    #[tool(description = "Resolve architecture evidence records by id.")]
    fn architecture_evidence(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_evidence", args.into_value())
    }

    #[tool(description = "Advanced: pack focus node neighborhood, co-located modules, and evidence for agent context.")]
    fn architecture_context_pack(
        &self,
        Parameters(args): Parameters<FreeformToolArgs>,
    ) -> Result<rmcp::model::CallToolResult, ErrorData> {
        self.invoke("architecture_context_pack", args.into_value())
    }
}

#[tool_handler]
impl ServerHandler for ArchitectureReaderMcp {
    fn get_info(&self) -> ServerInfo {
        // rmcp >=1.8: ServerInfo/Implementation are #[non_exhaustive] — use builders only.
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(SERVER_NAME, SERVER_VERSION)
                    .with_description(
                        "Rust-native MCP server for Spine / @sylphx/spine (modelcontextprotocol/rust-sdk rmcp)",
                    )
                    .with_website_url("https://sylphxai.github.io/spine/"),
            )
            .with_instructions(SERVER_INSTRUCTIONS)
    }
}

#[cfg(test)]
mod tests {
    use super::ArchitectureReaderMcp;
    use crate::tool_routes::PRIMARY_TOOLS;
    #[test]
    fn exposes_all_primary_tools() {
        let tools = ArchitectureReaderMcp::new().tool_router.list_all();
        let names: Vec<_> = tools.iter().map(|tool| tool.name.to_string()).collect();
        for tool in PRIMARY_TOOLS {
            assert!(names.contains(&tool.to_string()), "missing tool {tool}");
        }
    }

    #[test]
    fn get_info_uses_non_exhaustive_builder_apis() {
        use rmcp::ServerHandler;
        let info = ArchitectureReaderMcp::new().get_info();
        assert_eq!(info.server_info.name, super::SERVER_NAME);
        assert_eq!(info.server_info.version, super::SERVER_VERSION);
        assert!(info.instructions.as_deref().unwrap_or("").contains("Architecture Reader"));
        assert!(info.capabilities.tools.is_some());
    }
}
