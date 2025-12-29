//! @acp:module "Primer"
//! @acp:summary "AI context primer generation with value-based optimization"
//! @acp:domain daemon
//! @acp:layer service
//!
//! # Primer Generation
//!
//! This module implements intelligent context generation for AI agents:
//!
//! - **Multi-dimensional value scoring**: Sections are scored on safety, efficiency,
//!   accuracy, and base value dimensions
//! - **Phase-based selection**: Required → Conditionally Required → Safety-critical →
//!   Value-optimized
//! - **Dynamic modifiers**: Condition-based score adjustments based on project state
//! - **Token budget optimization**: Maximize value within token constraints
//! - **Capability filtering**: Include only sections relevant to the agent's capabilities

pub mod rendering;
pub mod scoring;
pub mod selection;
pub mod state;
pub mod types;

use acp::cache::Cache;

use rendering::PrimerRenderer;
use scoring::score_sections;
use selection::select_sections;
use state::ProjectState;
use types::{GeneratePrimerRequest, PrimerDefaults, PrimerSection};

/// Embedded primer defaults (from primers/primer.defaults.json)
const PRIMER_DEFAULTS_JSON: &str = include_str!("../../primers/primer.defaults.json");

/// Main primer generator
pub struct PrimerGenerator {
    defaults: PrimerDefaults,
}

#[allow(dead_code)]
impl PrimerGenerator {
    /// Create a new primer generator with embedded defaults
    pub fn new() -> Result<Self, PrimerError> {
        let defaults: PrimerDefaults = serde_json::from_str(PRIMER_DEFAULTS_JSON)
            .map_err(|e| PrimerError::ParseDefaults(e.to_string()))?;

        Ok(Self { defaults })
    }

    /// Create a primer generator with custom defaults
    pub fn with_defaults(defaults: PrimerDefaults) -> Self {
        Self { defaults }
    }

    /// Generate a primer for the given cache
    pub fn generate(&self, cache: &Cache, request: &GeneratePrimerRequest) -> PrimerResult {
        // Build project state from cache
        let state = ProjectState::from_cache(cache);

        // Get weights from preset
        let weights = request.preset.weights();

        // Score all sections
        let scored = score_sections(&self.defaults.sections, &state, &weights, true);

        // Select sections within budget
        let selection = select_sections(&scored, request);

        // Render selected sections
        let renderer = PrimerRenderer::new(request.format);
        let content = renderer
            .render(&selection.selected, cache)
            .unwrap_or_else(|e| format!("Error rendering primer: {}", e));

        PrimerResult {
            content,
            sections: selection.selected,
            tokens_used: selection.tokens_used,
            token_budget: request.token_budget,
            excluded_count: selection.excluded_count,
        }
    }

    /// Generate primer with default settings
    pub fn generate_default(&self, cache: &Cache) -> PrimerResult {
        self.generate(cache, &GeneratePrimerRequest::default())
    }

    /// Generate primer with custom budget
    pub fn generate_with_budget(&self, cache: &Cache, budget: usize) -> PrimerResult {
        let request = GeneratePrimerRequest {
            token_budget: budget,
            ..Default::default()
        };
        self.generate(cache, &request)
    }

    /// Generate primer with specific format
    pub fn generate_with_format(
        &self,
        cache: &Cache,
        budget: usize,
        format: OutputFormat,
    ) -> PrimerResult {
        let request = GeneratePrimerRequest {
            token_budget: budget,
            format,
            ..Default::default()
        };
        self.generate(cache, &request)
    }

    /// Generate primer with specific preset
    pub fn generate_with_preset(
        &self,
        cache: &Cache,
        budget: usize,
        preset: Preset,
    ) -> PrimerResult {
        let request = GeneratePrimerRequest {
            token_budget: budget,
            preset,
            ..Default::default()
        };
        self.generate(cache, &request)
    }

    /// Get the section definitions
    pub fn sections(&self) -> &[PrimerSection] {
        &self.defaults.sections
    }

    /// Get the embedded defaults
    pub fn defaults(&self) -> &PrimerDefaults {
        &self.defaults
    }

    /// Get defaults as JSON string
    pub fn defaults_json(&self) -> Result<String, PrimerError> {
        serde_json::to_string_pretty(&self.defaults)
            .map_err(|e| PrimerError::Serialize(e.to_string()))
    }
}

impl Default for PrimerGenerator {
    fn default() -> Self {
        Self::new().expect("Failed to load embedded primer defaults")
    }
}

/// Primer generation errors
#[derive(Debug)]
#[allow(dead_code)]
pub enum PrimerError {
    ParseDefaults(String),
    Serialize(String),
}

impl std::fmt::Display for PrimerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseDefaults(msg) => write!(f, "Failed to parse primer defaults: {}", msg),
            Self::Serialize(msg) => write!(f, "Failed to serialize: {}", msg),
        }
    }
}

impl std::error::Error for PrimerError {}

// Re-export commonly used types
pub use types::{GeneratePrimerRequest as PrimerRequest, OutputFormat, Preset, PrimerResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_defaults() {
        let generator = PrimerGenerator::new();
        assert!(generator.is_ok());

        let gen = generator.unwrap();
        assert!(!gen.sections().is_empty());
    }

    #[test]
    fn test_generate_default() {
        let generator = PrimerGenerator::default();
        let cache = Cache::new("test", ".");

        let result = generator.generate_default(&cache);

        // Should have some content
        assert!(!result.content.is_empty());
        // Should include required sections
        assert!(!result.sections.is_empty());
        // Should respect budget
        assert!(result.tokens_used <= result.token_budget);
    }

    #[test]
    fn test_generate_with_budget() {
        let generator = PrimerGenerator::default();
        let cache = Cache::new("test", ".");

        let result = generator.generate_with_budget(&cache, 100);

        // Small budget should limit sections
        assert!(result.tokens_used <= 100);
    }

    #[test]
    fn test_generate_compact_format() {
        let generator = PrimerGenerator::default();
        let cache = Cache::new("test", ".");

        let result = generator.generate_with_format(&cache, 4000, OutputFormat::Compact);

        // Compact format should be shorter
        assert!(!result.content.is_empty());
    }

    #[test]
    fn test_defaults_json() {
        let generator = PrimerGenerator::default();
        let json = generator.defaults_json();

        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("\"sections\""));
    }
}
