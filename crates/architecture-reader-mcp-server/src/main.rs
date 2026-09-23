use architecture_reader_mcp_server::{ArchitectureReaderMcp, SERVER_VERSION};
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("doctor") {
        eprintln!(
            "Spine Rust MCP server {SERVER_VERSION} ({})",
            architecture_reader_core::ENGINE_NAME
        );
        eprintln!("engine: in-process (architecture-reader-core)");
        return Ok(());
    }

    let service = ArchitectureReaderMcp::new()
        .serve(rmcp::transport::stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}
