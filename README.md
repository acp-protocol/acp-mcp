# ACP MCP Server

Model Context Protocol (MCP) server for the AI Context Protocol.

## Installation

```bash
# Via cargo
cargo install acp-mcp

# Or via ACP CLI
acp install mcp
```

## Usage

The MCP server runs over stdio for integration with AI tools like Claude Desktop.

```bash
# Run MCP server
acp-mcp

# With custom project root
acp-mcp -C /path/to/project

# With debug logging
acp-mcp --log-level debug
```

## Claude Desktop Integration

Add to your Claude Desktop configuration (`claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "acp": {
      "command": "acp-mcp",
      "args": ["-C", "/path/to/your/project"]
    }
  }
}
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `acp_get_architecture` | Get project overview and structure |
| `acp_get_file_context` | Get file details with relationships |
| `acp_get_symbol_context` | Get symbol analysis with call graphs |
| `acp_get_domain_files` | Query files by domain |
| `acp_check_constraints` | Verify constraint compliance |
| `acp_get_hotpaths` | Find critical/frequently-called symbols |
| `acp_expand_variable` | Resolve variable values |
| `acp_generate_primer` | Generate optimized AI context |

## Requirements

The MCP server reads ACP files from the project root:
- `.acp/acp.cache.json` - Indexed cache (required)
- `.acp/acp.vars.json` - Variables (optional)
- `.acp.config.json` - Configuration (optional)

Generate these with the ACP CLI:
```bash
acp index
```

## License

MIT
