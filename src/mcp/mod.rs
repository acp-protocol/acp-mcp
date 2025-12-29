//! @acp:module "MCP Server"
//! @acp:summary "Model Context Protocol server for AI agent integration"
//! @acp:domain daemon
//! @acp:layer transport
//!
//! Provides MCP server capabilities for AI agents like Claude Desktop.
//! Exposes ACP cache, symbols, and domains as MCP tools and resources.

mod service;
mod tools;

pub use service::AcpMcpService;

use rmcp::ServiceExt;
use std::path::Path;
use tokio::io::{stdin, stdout};
use tracing::{error, info};

use crate::state::AppState;

/// Run the MCP server over stdio
pub async fn run_stdio_server(project_root: &Path) -> anyhow::Result<()> {
    info!("Starting MCP server over stdio");

    // Load ACP state
    let state = AppState::load(project_root).await?;

    {
        let cache = state.cache_async().await;
        info!(
            "MCP server loaded cache with {} files, {} symbols",
            cache.files.len(),
            cache.symbols.len()
        );
    }

    // Create MCP service
    let service = AcpMcpService::new(state);

    // Create stdio transport
    let transport = (stdin(), stdout());

    // Serve MCP protocol
    info!("MCP server ready, waiting for requests...");
    match service.serve(transport).await {
        Ok(server) => {
            server.waiting().await?;
            info!("MCP server shutdown");
        }
        Err(e) => {
            error!("MCP server error: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
