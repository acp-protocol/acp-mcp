//! @acp:module "MCP Service"
//! @acp:summary "Core MCP service implementation using rmcp SDK"
//! @acp:domain daemon
//! @acp:layer service
//!
//! Implements the ServerHandler trait for ACP MCP integration.
//! Provides tools and resources for AI agent context generation.

use rmcp::{model::*, schemars, ErrorData as McpError, ServerHandler};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;

/// ACP MCP Service - exposes codebase context to AI agents
#[derive(Clone)]
pub struct AcpMcpService {
    state: AppState,
}

// Tool parameter types
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetFileContextParams {
    /// Path to the file (relative to project root)
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSymbolContextParams {
    /// Name of the symbol to look up
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetDomainFilesParams {
    /// Name of the domain
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CheckConstraintsParams {
    /// Path to the file to check constraints for
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExpandVariableParams {
    /// Variable name to expand (e.g., "SYM_AuthService")
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GeneratePrimerParams {
    /// Maximum token budget for the primer (default: 4000)
    #[serde(default = "default_token_budget")]
    pub token_budget: usize,
    /// Output format: "markdown", "compact", or "json" (default: "markdown")
    #[serde(default = "default_format")]
    pub format: String,
    /// Weight preset: "safe", "efficient", "accurate", or "balanced" (default: "balanced")
    #[serde(default = "default_preset")]
    pub preset: String,
    /// Available capabilities (default: ["shell", "file-read", "file-write"])
    #[serde(default = "default_capabilities")]
    pub capabilities: Vec<String>,
    /// Filter by categories (optional)
    #[serde(default)]
    pub categories: Option<Vec<String>>,
    /// Filter by tags (optional)
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Force include specific section IDs (optional)
    #[serde(default)]
    pub force_include: Vec<String>,
}

fn default_token_budget() -> usize {
    4000
}

fn default_format() -> String {
    "markdown".to_string()
}

fn default_preset() -> String {
    "balanced".to_string()
}

fn default_capabilities() -> Vec<String> {
    vec![
        "shell".to_string(),
        "file-read".to_string(),
        "file-write".to_string(),
    ]
}

/// RFC-0015: Context operation for acp_context tool
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetContextParams {
    /// Operation type: "create", "modify", "debug", or "explore"
    pub operation: String,
    /// For create: directory path. For modify/debug: file path. For explore: optional domain.
    pub target: Option<String>,
    /// For modify: whether to find files that use this file
    #[serde(default)]
    pub find_usages: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[allow(dead_code)]
struct EmptyParams {}

// Tool response types for structured output
#[derive(Debug, Serialize, JsonSchema)]
pub struct ArchitectureResponse {
    pub project_name: String,
    pub total_files: usize,
    pub total_symbols: usize,
    pub domains: Vec<DomainSummary>,
    pub languages: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DomainSummary {
    pub name: String,
    pub description: Option<String>,
    pub file_count: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HotpathSymbol {
    pub name: String,
    pub caller_count: usize,
    pub file: String,
    pub symbol_type: String,
}

/// Convert a schemars Schema to a JsonObject for rmcp Tool
fn schema_to_json_object<T: JsonSchema>() -> Arc<serde_json::Map<String, serde_json::Value>> {
    let schema = schemars::schema_for!(T);
    let json_value = serde_json::to_value(&schema).unwrap_or_default();
    if let serde_json::Value::Object(map) = json_value {
        Arc::new(map)
    } else {
        Arc::new(serde_json::Map::new())
    }
}

fn empty_schema() -> Arc<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();
    map.insert(
        "type".to_string(),
        serde_json::Value::String("object".to_string()),
    );
    Arc::new(map)
}

impl AcpMcpService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    fn build_tools() -> Vec<Tool> {
        vec![
            Tool::new(
                "acp_get_architecture",
                "Get an overview of the codebase architecture including domains, files, symbols, and structure. Use this first to understand the project layout.",
                empty_schema(),
            ),
            Tool::new(
                "acp_get_file_context",
                "Get detailed context for a specific file including exports, imports, symbols, constraints, and relationships.",
                schema_to_json_object::<GetFileContextParams>(),
            ),
            Tool::new(
                "acp_get_symbol_context",
                "Get detailed context for a symbol including its definition, callers, callees, constraints, and domain membership.",
                schema_to_json_object::<GetSymbolContextParams>(),
            ),
            Tool::new(
                "acp_get_domain_files",
                "Get all files belonging to a specific domain with their metadata.",
                schema_to_json_object::<GetDomainFilesParams>(),
            ),
            Tool::new(
                "acp_check_constraints",
                "Check what constraints (lock levels, style rules, behavior requirements) apply to a file or its symbols.",
                schema_to_json_object::<CheckConstraintsParams>(),
            ),
            Tool::new(
                "acp_get_hotpaths",
                "Get the most frequently called symbols in the codebase - the 'hotpaths' that are critical to understand.",
                empty_schema(),
            ),
            Tool::new(
                "acp_expand_variable",
                "Expand an ACP variable (like $SYM_AuthService, $FILE_config, $DOM_core) to its full context.",
                schema_to_json_object::<ExpandVariableParams>(),
            ),
            Tool::new(
                "acp_generate_primer",
                "Generate an optimized context primer for the codebase within a token budget. Returns the most important information about the project structure, key files, and critical symbols.",
                schema_to_json_object::<GeneratePrimerParams>(),
            ),
            Tool::new(
                "acp_context",
                "RFC-0015: Get operation-specific context for AI agent tasks. Operations: 'create' (naming conventions for new files), 'modify' (constraints/importers for existing files), 'debug' (related files/symbols), 'explore' (project overview/domains).",
                schema_to_json_object::<GetContextParams>(),
            ),
        ]
    }

    /// Get codebase architecture overview
    async fn handle_get_architecture(&self) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let domains: Vec<DomainSummary> = cache
            .domains
            .iter()
            .map(|(name, domain)| DomainSummary {
                name: name.clone(),
                description: domain.description.clone(),
                file_count: domain.files.len(),
            })
            .collect();

        let languages: Vec<String> = cache
            .files
            .values()
            .map(|f| format!("{:?}", f.language))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let response = ArchitectureResponse {
            project_name: cache.project.name.clone(),
            total_files: cache.files.len(),
            total_symbols: cache.symbols.len(),
            domains,
            languages,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get file context with all metadata
    async fn handle_get_file_context(&self, path: String) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let file = cache
            .get_file(&path)
            .ok_or_else(|| McpError::invalid_params(format!("File not found: {}", path), None))?;

        let json = serde_json::to_string_pretty(file)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get symbol context with relationships
    async fn handle_get_symbol_context(&self, name: String) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let symbol = cache
            .symbols
            .get(&name)
            .ok_or_else(|| McpError::invalid_params(format!("Symbol not found: {}", name), None))?;

        // Get callers and callees from graph (if available)
        let (callers, callees) = if let Some(ref graph) = cache.graph {
            (
                graph.reverse.get(&name).cloned().unwrap_or_default(),
                graph.forward.get(&name).cloned().unwrap_or_default(),
            )
        } else {
            (Vec::new(), Vec::new())
        };

        #[derive(Serialize)]
        struct SymbolContext {
            symbol: acp::cache::SymbolEntry,
            callers: Vec<String>,
            callees: Vec<String>,
        }

        let context = SymbolContext {
            symbol: symbol.clone(),
            callers,
            callees,
        };

        let json = serde_json::to_string_pretty(&context)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get files in a domain
    async fn handle_get_domain_files(&self, name: String) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let domain = cache
            .domains
            .get(&name)
            .ok_or_else(|| McpError::invalid_params(format!("Domain not found: {}", name), None))?;

        let json = serde_json::to_string_pretty(domain)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Check constraints for a file
    async fn handle_check_constraints(&self, path: String) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let json = if let Some(ref constraints) = cache.constraints {
            if let Some(c) = constraints.by_file.get(&path) {
                serde_json::to_string_pretty(c)
                    .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?
            } else {
                r#"{"message": "No constraints found for this file"}"#.to_string()
            }
        } else {
            r#"{"message": "No constraints defined in cache"}"#.to_string()
        };

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get hotpath symbols (most called)
    async fn handle_get_hotpaths(&self) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let hotpaths = if let Some(ref graph) = cache.graph {
            // Count callers for each symbol
            let mut symbol_callers: Vec<(&String, usize)> = graph
                .reverse
                .iter()
                .map(|(name, callers)| (name, callers.len()))
                .collect();

            // Sort by caller count descending
            symbol_callers.sort_by(|a, b| b.1.cmp(&a.1));

            // Take top 20
            symbol_callers
                .into_iter()
                .take(20)
                .filter_map(|(name, caller_count)| {
                    cache.symbols.get(name).map(|sym| HotpathSymbol {
                        name: name.clone(),
                        caller_count,
                        file: sym.file.clone(),
                        symbol_type: format!("{:?}", sym.symbol_type),
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let json = serde_json::to_string_pretty(&hotpaths)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Expand a variable reference
    async fn handle_expand_variable(&self, name: String) -> Result<CallToolResult, McpError> {
        let vars_guard = self.state.vars().await;

        let vars = vars_guard
            .as_ref()
            .ok_or_else(|| McpError::invalid_params("No vars file loaded".to_string(), None))?;

        let variable = vars.variables.get(&name).ok_or_else(|| {
            McpError::invalid_params(format!("Variable not found: {}", name), None)
        })?;

        let json = serde_json::to_string_pretty(variable)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Generate a primer for AI context using value-based optimization
    async fn handle_generate_primer(
        &self,
        params: GeneratePrimerParams,
    ) -> Result<CallToolResult, McpError> {
        use crate::primer::{OutputFormat, Preset, PrimerGenerator, PrimerRequest};

        let cache = self.state.cache_async().await;

        // Create primer generator
        let generator = PrimerGenerator::default();

        // Build request from params
        let request = PrimerRequest {
            token_budget: params.token_budget,
            format: OutputFormat::from_str(&params.format),
            preset: Preset::from_str(&params.preset),
            capabilities: params.capabilities,
            categories: params.categories,
            tags: params.tags,
            force_include: params.force_include,
        };

        // Generate primer
        let result = generator.generate(&cache, &request);

        // Build response with metadata
        #[derive(Serialize)]
        struct PrimerResponse {
            content: String,
            tokens_used: usize,
            token_budget: usize,
            sections_included: usize,
            sections_excluded: usize,
        }

        let response = PrimerResponse {
            content: result.content,
            tokens_used: result.tokens_used,
            token_budget: result.token_budget,
            sections_included: result.sections.len(),
            sections_excluded: result.excluded_count,
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// RFC-0015: Handle acp_context tool - operation-specific context
    async fn handle_get_context(&self, params: GetContextParams) -> Result<CallToolResult, McpError> {
        let cache = self.state.cache_async().await;

        let result = match params.operation.as_str() {
            "create" => {
                let directory = params.target.ok_or_else(|| {
                    McpError::invalid_params("'target' (directory path) required for create operation".to_string(), None)
                })?;
                self.generate_create_context(&cache, &directory)
            }
            "modify" => {
                let file = params.target.ok_or_else(|| {
                    McpError::invalid_params("'target' (file path) required for modify operation".to_string(), None)
                })?;
                self.generate_modify_context(&cache, &file, params.find_usages)
            }
            "debug" => {
                let target = params.target.ok_or_else(|| {
                    McpError::invalid_params("'target' (file or symbol) required for debug operation".to_string(), None)
                })?;
                self.generate_debug_context(&cache, &target)
            }
            "explore" => {
                self.generate_explore_context(&cache, params.target.as_deref())
            }
            _ => {
                return Err(McpError::invalid_params(
                    format!("Unknown operation: {}. Use: create, modify, debug, or explore", params.operation),
                    None,
                ));
            }
        };

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("JSON error: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Generate context for creating new files
    fn generate_create_context(&self, cache: &acp::cache::Cache, directory: &str) -> serde_json::Value {
        // Find naming conventions for this directory
        let naming = cache
            .conventions
            .file_naming
            .iter()
            .find(|n| n.directory == directory)
            .or_else(|| {
                cache.conventions.file_naming
                    .iter()
                    .filter(|n| directory.starts_with(&n.directory))
                    .max_by_key(|n| n.directory.len())
            });

        // Detect primary language in directory
        let language = self.detect_directory_language(cache, directory);

        // Get import style from conventions
        let import_style = cache.conventions.imports.as_ref().map(|i| {
            serde_json::json!({
                "module_system": i.module_system.as_ref()
                    .map(|m| format!("{:?}", m).to_lowercase())
                    .unwrap_or_else(|| "esm".to_string()),
                "path_style": i.path_style.as_ref()
                    .map(|p| format!("{:?}", p).to_lowercase())
                    .unwrap_or_else(|| "relative".to_string()),
                "index_exports": i.index_exports
            })
        });

        // Find similar files in the directory
        let similar_files: Vec<&String> = cache.files.keys()
            .filter(|p| {
                std::path::Path::new(p)
                    .parent()
                    .map(|parent| parent.to_string_lossy() == directory)
                    .unwrap_or(false)
            })
            .take(5)
            .collect();

        serde_json::json!({
            "operation": "create",
            "directory": directory,
            "language": language,
            "naming_convention": naming.map(|n| serde_json::json!({
                "pattern": n.pattern,
                "confidence": n.confidence,
                "examples": n.examples
            })),
            "import_style": import_style,
            "similar_files": similar_files,
            "recommended_pattern": naming.map(|n| &n.pattern)
        })
    }

    /// Generate context for modifying existing files
    fn generate_modify_context(&self, cache: &acp::cache::Cache, file: &str, _find_usages: bool) -> serde_json::Value {
        let file_entry = cache.files.get(file);

        // Get importers from the file entry
        let importers = file_entry
            .map(|f| &f.imported_by)
            .map(|v| v.iter().collect::<Vec<_>>())
            .unwrap_or_default();

        // Get file constraints
        let constraints = cache.constraints.as_ref().and_then(|c| {
            c.by_file.get(file).and_then(|fc| {
                fc.mutation.as_ref().map(|m| serde_json::json!({
                    "level": format!("{:?}", m.level).to_lowercase(),
                    "reason": m.reason
                }))
            })
        });

        // Get symbols in this file
        let symbols = file_entry.map(|f| &f.exports).cloned().unwrap_or_default();

        // Get domain
        let domain = cache.domains.iter()
            .find(|(_, d)| d.files.contains(&file.to_string()))
            .map(|(name, _)| name.clone());

        serde_json::json!({
            "operation": "modify",
            "file": file,
            "importers": importers,
            "importer_count": importers.len(),
            "constraints": constraints,
            "symbols": symbols,
            "domain": domain
        })
    }

    /// Generate context for debugging
    fn generate_debug_context(&self, cache: &acp::cache::Cache, target: &str) -> serde_json::Value {
        // Target could be a file or symbol
        let (file_path, symbols_info) = if cache.files.contains_key(target) {
            // It's a file
            let file = cache.files.get(target).unwrap();
            let symbols: Vec<serde_json::Value> = file.exports.iter()
                .filter_map(|name| cache.symbols.get(name))
                .map(|s| serde_json::json!({
                    "name": s.name,
                    "type": format!("{:?}", s.symbol_type).to_lowercase(),
                    "purpose": s.purpose
                }))
                .collect();
            (target.to_string(), symbols)
        } else if let Some(symbol) = cache.symbols.get(target) {
            // It's a symbol
            (symbol.file.clone(), vec![serde_json::json!({
                "name": symbol.name,
                "type": format!("{:?}", symbol.symbol_type).to_lowercase(),
                "purpose": symbol.purpose
            })])
        } else {
            return serde_json::json!({
                "operation": "debug",
                "error": format!("Target not found: {}. Provide a file path or symbol name.", target)
            });
        };

        // Get related files (imports)
        let related_files = cache.files.get(&file_path)
            .map(|f| &f.imports)
            .cloned()
            .unwrap_or_default();

        // Get hotpaths through this code
        let hotpaths: Vec<String> = if let Some(ref graph) = cache.graph {
            graph.reverse.iter()
                .filter(|(name, callers)| {
                    callers.len() >= 3 &&
                    (name.as_str() == target || file_path.contains(name.as_str()))
                })
                .map(|(name, _)| name.clone())
                .take(5)
                .collect()
        } else {
            Vec::new()
        };

        serde_json::json!({
            "operation": "debug",
            "target": target,
            "file": file_path,
            "related_files": related_files,
            "symbols": symbols_info,
            "hotpaths": hotpaths
        })
    }

    /// Generate context for exploring the codebase
    fn generate_explore_context(&self, cache: &acp::cache::Cache, domain_filter: Option<&str>) -> serde_json::Value {
        let stats = serde_json::json!({
            "files": cache.stats.files,
            "symbols": cache.stats.symbols,
            "lines": cache.stats.lines,
            "primary_language": cache.stats.primary_language,
            "annotation_coverage": cache.stats.annotation_coverage
        });

        // Get domains
        let domains: Vec<serde_json::Value> = cache.domains.iter()
            .filter(|(name, _)| domain_filter.is_none_or(|f| name.contains(f)))
            .map(|(name, d)| serde_json::json!({
                "name": name,
                "file_count": d.files.len(),
                "symbol_count": d.symbols.len(),
                "description": d.description
            }))
            .collect();

        // Get key files (most imported)
        let mut key_files: Vec<(&String, usize)> = cache.files.iter()
            .map(|(path, entry)| (path, entry.imported_by.len()))
            .collect();
        key_files.sort_by(|a, b| b.1.cmp(&a.1));
        let key_files: Vec<&String> = key_files.iter().take(10).map(|(p, _)| *p).collect();

        serde_json::json!({
            "operation": "explore",
            "domain_filter": domain_filter,
            "stats": stats,
            "domains": domains,
            "key_files": key_files
        })
    }

    /// Detect the primary language in a directory
    fn detect_directory_language(&self, cache: &acp::cache::Cache, directory: &str) -> Option<String> {
        use std::collections::HashMap;

        let mut lang_counts: HashMap<String, usize> = HashMap::new();

        for (path, file) in &cache.files {
            let parent = std::path::Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            if parent == directory || parent.starts_with(&format!("{}/", directory)) {
                let lang = format!("{:?}", file.language).to_lowercase();
                *lang_counts.entry(lang).or_insert(0) += 1;
            }
        }

        lang_counts.into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(lang, _)| lang)
    }

    /// Parse tool arguments from request
    fn parse_args<T: for<'de> Deserialize<'de>>(
        args: Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<T, McpError> {
        let value = serde_json::Value::Object(args.unwrap_or_default());
        serde_json::from_value(value).map_err(|e| McpError::invalid_params(e.to_string(), None))
    }
}

#[allow(clippy::manual_async_fn)]
impl ServerHandler for AcpMcpService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "ACP (AI Context Protocol) server providing codebase context for AI agents. \
                 Use acp_get_architecture first to understand the project structure, then \
                 use other tools to explore specific files, symbols, and domains."
                    .to_string(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                tools: Self::build_tools(),
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let tool_name: &str = &request.name;
            match tool_name {
                "acp_get_architecture" => self.handle_get_architecture().await,
                "acp_get_file_context" => {
                    let params: GetFileContextParams = Self::parse_args(request.arguments)?;
                    self.handle_get_file_context(params.path).await
                }
                "acp_get_symbol_context" => {
                    let params: GetSymbolContextParams = Self::parse_args(request.arguments)?;
                    self.handle_get_symbol_context(params.name).await
                }
                "acp_get_domain_files" => {
                    let params: GetDomainFilesParams = Self::parse_args(request.arguments)?;
                    self.handle_get_domain_files(params.name).await
                }
                "acp_check_constraints" => {
                    let params: CheckConstraintsParams = Self::parse_args(request.arguments)?;
                    self.handle_check_constraints(params.path).await
                }
                "acp_get_hotpaths" => self.handle_get_hotpaths().await,
                "acp_expand_variable" => {
                    let params: ExpandVariableParams = Self::parse_args(request.arguments)?;
                    self.handle_expand_variable(params.name).await
                }
                "acp_generate_primer" => {
                    let params: GeneratePrimerParams = Self::parse_args(request.arguments)?;
                    self.handle_generate_primer(params).await
                }
                "acp_context" => {
                    let params: GetContextParams = Self::parse_args(request.arguments)?;
                    self.handle_get_context(params).await
                }
                _ => Err(McpError::invalid_params(
                    format!("Unknown tool: {}", request.name),
                    None,
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acp::cache::Cache;

    fn create_test_service() -> AcpMcpService {
        let cache = Cache::new("test-project", ".");
        let state = crate::state::AppState::for_testing(cache, None);
        AcpMcpService::new(state)
    }

    #[tokio::test]
    async fn test_generate_primer_default_params() {
        let service = create_test_service();

        let params = GeneratePrimerParams {
            token_budget: 4000,
            format: "markdown".to_string(),
            preset: "balanced".to_string(),
            capabilities: vec!["file-read".to_string()],
            categories: None,
            tags: None,
            force_include: vec![],
        };

        let result = service.handle_generate_primer(params).await;
        assert!(result.is_ok(), "Primer generation should succeed");

        let call_result = result.unwrap();
        assert!(!call_result.content.is_empty(), "Should have content");

        // Verify content is valid JSON
        if let Some(content) = call_result.content.first() {
            if let Some(text) = content.as_text() {
                let parsed: Result<serde_json::Value, _> = serde_json::from_str(text.text.as_str());
                assert!(parsed.is_ok(), "Content should be valid JSON");

                let json = parsed.unwrap();
                assert!(json.get("content").is_some(), "Should have content field");
                assert!(
                    json.get("tokens_used").is_some(),
                    "Should have tokens_used field"
                );
                assert!(
                    json.get("token_budget").is_some(),
                    "Should have token_budget field"
                );
            }
        }
    }

    #[tokio::test]
    async fn test_generate_primer_compact_format() {
        let service = create_test_service();

        let params = GeneratePrimerParams {
            token_budget: 2000,
            format: "compact".to_string(),
            preset: "safe".to_string(),
            capabilities: vec!["shell".to_string(), "file-read".to_string()],
            categories: None,
            tags: None,
            force_include: vec![],
        };

        let result = service.handle_generate_primer(params).await;
        assert!(result.is_ok(), "Compact primer should succeed");
    }

    #[tokio::test]
    async fn test_generate_primer_with_budget() {
        let service = create_test_service();

        let params = GeneratePrimerParams {
            token_budget: 500,
            format: "markdown".to_string(),
            preset: "balanced".to_string(),
            capabilities: vec![],
            categories: None,
            tags: None,
            force_include: vec![],
        };

        let result = service.handle_generate_primer(params).await;
        assert!(result.is_ok(), "Small budget primer should succeed");

        // Verify we respect the budget
        if let Some(content) = result.unwrap().content.first() {
            if let Some(text) = content.as_text() {
                let json: serde_json::Value = serde_json::from_str(text.text.as_str()).unwrap();
                let tokens_used = json
                    .get("tokens_used")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                assert!(
                    tokens_used <= 500,
                    "Tokens used {} should be <= budget 500",
                    tokens_used
                );
            }
        }
    }

    #[tokio::test]
    async fn test_acp_context_explore() {
        let service = create_test_service();

        let params = GetContextParams {
            operation: "explore".to_string(),
            target: None,
            find_usages: false,
        };

        let result = service.handle_get_context(params).await;
        assert!(result.is_ok(), "Explore context should succeed");

        if let Some(content) = result.unwrap().content.first() {
            if let Some(text) = content.as_text() {
                let json: serde_json::Value = serde_json::from_str(text.text.as_str()).unwrap();
                assert_eq!(json.get("operation").and_then(|v| v.as_str()), Some("explore"));
                assert!(json.get("stats").is_some(), "Should have stats");
                assert!(json.get("domains").is_some(), "Should have domains");
            }
        }
    }

    #[tokio::test]
    async fn test_acp_context_create() {
        let service = create_test_service();

        let params = GetContextParams {
            operation: "create".to_string(),
            target: Some("src".to_string()),
            find_usages: false,
        };

        let result = service.handle_get_context(params).await;
        assert!(result.is_ok(), "Create context should succeed");

        if let Some(content) = result.unwrap().content.first() {
            if let Some(text) = content.as_text() {
                let json: serde_json::Value = serde_json::from_str(text.text.as_str()).unwrap();
                assert_eq!(json.get("operation").and_then(|v| v.as_str()), Some("create"));
                assert_eq!(json.get("directory").and_then(|v| v.as_str()), Some("src"));
            }
        }
    }

    #[tokio::test]
    async fn test_acp_context_invalid_operation() {
        let service = create_test_service();

        let params = GetContextParams {
            operation: "invalid".to_string(),
            target: None,
            find_usages: false,
        };

        let result = service.handle_get_context(params).await;
        assert!(result.is_err(), "Invalid operation should fail");
    }

    #[tokio::test]
    async fn test_acp_context_missing_target() {
        let service = create_test_service();

        let params = GetContextParams {
            operation: "modify".to_string(),
            target: None,
            find_usages: false,
        };

        let result = service.handle_get_context(params).await;
        assert!(result.is_err(), "Modify without target should fail");
    }
}
