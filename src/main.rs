#![forbid(unsafe_code)]

//! @acp:module "ACP MCP Server Entry Point"
//! @acp:summary "Main entry point for the ACP MCP server"
//! @acp:domain mcp
//! @acp:layer application
//!
//! The ACP MCP server provides Model Context Protocol integration for AI tools.
//! It exposes ACP cache, symbols, and domains as MCP tools for Claude Desktop
//! and other MCP-compatible AI agents.

use std::path::PathBuf;

use clap::Parser;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod mcp;
mod primer;
mod state;

/// ACP MCP Server - Model Context Protocol for AI tools
#[derive(Parser, Debug)]
#[command(name = "acp-mcp")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Project root directory
    #[arg(long, short = 'C')]
    directory: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging (to stderr so stdout is free for MCP)
    init_logging(&cli.log_level);

    // Determine project root
    let project_root = cli
        .directory
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current directory"));

    info!("ACP MCP Server starting");
    info!("Project root: {}", project_root.display());

    // Run MCP server over stdio
    mcp::run_stdio_server(&project_root).await
}

fn init_logging(level: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();
}
