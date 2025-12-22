//! @acp:module "Primer Types"
//! @acp:summary "Core data structures for primer generation"
//! @acp:domain daemon
//! @acp:layer model

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Output format for primer rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Markdown,
    Compact,
    Json,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "compact" => Self::Compact,
            "json" => Self::Json,
            _ => Self::Markdown,
        }
    }
}

/// Preset weight configurations for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    Safe,
    Efficient,
    Accurate,
    #[default]
    Balanced,
}

impl Preset {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "safe" => Self::Safe,
            "efficient" => Self::Efficient,
            "accurate" => Self::Accurate,
            _ => Self::Balanced,
        }
    }

    pub fn weights(&self) -> DimensionWeights {
        match self {
            Self::Safe => DimensionWeights {
                safety: 2.5,
                efficiency: 0.8,
                accuracy: 1.0,
                base: 0.8,
            },
            Self::Efficient => DimensionWeights {
                safety: 1.2,
                efficiency: 2.0,
                accuracy: 0.9,
                base: 0.8,
            },
            Self::Accurate => DimensionWeights {
                safety: 1.2,
                efficiency: 0.9,
                accuracy: 2.0,
                base: 0.8,
            },
            Self::Balanced => DimensionWeights::default(),
        }
    }
}

/// Weights for multi-dimensional value calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionWeights {
    pub safety: f64,
    pub efficiency: f64,
    pub accuracy: f64,
    pub base: f64,
}

impl Default for DimensionWeights {
    fn default() -> Self {
        Self {
            safety: 1.5,
            efficiency: 1.0,
            accuracy: 1.0,
            base: 1.0,
        }
    }
}

/// Multi-dimensional value scoring for section selection
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectionValue {
    /// How critical for preventing harmful AI actions (0-100)
    #[serde(default)]
    pub safety: i32,
    /// How much it saves future tokens/queries (0-100)
    #[serde(default)]
    pub efficiency: i32,
    /// How much it improves response quality (0-100)
    #[serde(default)]
    pub accuracy: i32,
    /// Baseline value independent of dimensions (0-100)
    #[serde(default = "default_base")]
    pub base: i32,
    /// Dynamic modifiers based on project state
    #[serde(default)]
    pub modifiers: Vec<ValueModifier>,
}

fn default_base() -> i32 {
    50
}

impl SectionValue {
    /// Calculate weighted score with optional modifiers applied
    pub fn weighted_score(&self, weights: &DimensionWeights) -> f64 {
        (self.safety as f64 * weights.safety)
            + (self.efficiency as f64 * weights.efficiency)
            + (self.accuracy as f64 * weights.accuracy)
            + (self.base as f64 * weights.base)
    }
}

/// Conditional modifier that adjusts section value based on project state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueModifier {
    /// Expression evaluated against project state (e.g., "constraints.frozenCount > 0")
    pub condition: String,
    /// Add this amount to score
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<i32>,
    /// Multiply score by this amount
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiply: Option<f64>,
    /// Override score to this value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<i32>,
    /// Which dimension(s) to modify
    #[serde(default = "default_dimension")]
    pub dimension: ModifierDimension,
    /// Human-readable explanation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

fn default_dimension() -> ModifierDimension {
    ModifierDimension::All
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModifierDimension {
    Safety,
    Efficiency,
    Accuracy,
    Base,
    #[default]
    All,
}

/// Token count specification - either fixed or dynamic
#[derive(Debug, Clone)]
pub enum TokenCount {
    Fixed(usize),
    Dynamic,
}

impl Default for TokenCount {
    fn default() -> Self {
        Self::Fixed(30)
    }
}

impl TokenCount {
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic)
    }

    pub fn fixed_value(&self) -> Option<usize> {
        match self {
            Self::Fixed(n) => Some(*n),
            Self::Dynamic => None,
        }
    }
}

impl Serialize for TokenCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Fixed(n) => serializer.serialize_u64(*n as u64),
            Self::Dynamic => serializer.serialize_str("dynamic"),
        }
    }
}

impl<'de> Deserialize<'de> for TokenCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};

        struct TokenCountVisitor;

        impl<'de> Visitor<'de> for TokenCountVisitor {
            type Value = TokenCount;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a number or the string \"dynamic\"")
            }

            fn visit_u64<E>(self, value: u64) -> Result<TokenCount, E>
            where
                E: de::Error,
            {
                Ok(TokenCount::Fixed(value as usize))
            }

            fn visit_i64<E>(self, value: i64) -> Result<TokenCount, E>
            where
                E: de::Error,
            {
                if value >= 0 {
                    Ok(TokenCount::Fixed(value as usize))
                } else {
                    Err(de::Error::custom("negative token count"))
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<TokenCount, E>
            where
                E: de::Error,
            {
                if value == "dynamic" {
                    Ok(TokenCount::Dynamic)
                } else {
                    Err(de::Error::custom(format!("unknown token type: {}", value)))
                }
            }
        }

        deserializer.deserialize_any(TokenCountVisitor)
    }
}

/// Data source configuration for dynamic sections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectionData {
    /// Data path (e.g., "cache.constraints.by_lock_level")
    pub source: String,
    /// Fields to extract from each item
    #[serde(default)]
    pub fields: Vec<String>,
    /// Filter criteria
    #[serde(default)]
    pub filter: Option<DataFilter>,
    /// Field to sort by
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
    /// Sort order
    #[serde(default)]
    pub sort_order: SortOrder,
    /// Maximum items to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,
    /// Estimated tokens per item
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_tokens: Option<usize>,
    /// What to do when empty
    #[serde(default)]
    pub empty_behavior: EmptyBehavior,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DataFilter {
    /// Include only these values
    Include(Vec<String>),
    /// Filter expression object
    Expression(HashMap<String, serde_json::Value>),
}

impl Default for DataFilter {
    fn default() -> Self {
        Self::Include(vec![])
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmptyBehavior {
    #[default]
    Exclude,
    Placeholder,
    Error,
}

/// Format template for rendering sections
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FormatTemplate {
    /// Handlebars template string (for static sections)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// Header text before items (for list sections)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// Footer text after items
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<String>,
    /// Template for each item in a list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_template: Option<String>,
    /// Separator between items
    #[serde(default = "default_separator")]
    pub separator: String,
    /// Template when no items
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_template: Option<String>,
}

fn default_separator() -> String {
    "\n".to_string()
}

/// Format templates for different output formats
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectionFormats {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<FormatTemplate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compact: Option<FormatTemplate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json: Option<FormatTemplate>,
}

impl SectionFormats {
    pub fn get(&self, format: OutputFormat) -> Option<&FormatTemplate> {
        match format {
            OutputFormat::Markdown => self.markdown.as_ref(),
            OutputFormat::Compact => self.compact.as_ref(),
            OutputFormat::Json => self.json.as_ref(),
        }
    }
}

/// A primer section definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimerSection {
    /// Unique section identifier
    pub id: String,
    /// Human-readable name
    #[serde(default)]
    pub name: String,
    /// What this section provides
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Category this section belongs to
    pub category: String,
    /// Priority within category (lower = higher priority)
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// Token cost for this section
    #[serde(default)]
    pub tokens: TokenCount,
    /// Multi-dimensional value scoring
    #[serde(default)]
    pub value: SectionValue,
    /// Always include regardless of budget
    #[serde(default)]
    pub required: bool,
    /// Condition expression that makes this required
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_if: Option<String>,
    /// Required capabilities (ANY of these)
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Required capabilities (ALL of these)
    #[serde(default)]
    pub capabilities_all: Vec<String>,
    /// Section IDs that must be included before this one
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Section IDs that cannot be included with this one
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    /// Data source for dynamic sections
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<SectionData>,
    /// Format templates
    #[serde(default)]
    pub formats: SectionFormats,
    /// Tags for filtering
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_priority() -> i32 {
    50
}

/// Category definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_constraints: Option<CategoryBudget>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CategoryBudget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_percent: Option<f64>,
}

/// Capability definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
}

/// Complete primer defaults file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimerDefaults {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PrimerMetadata>,
    #[serde(default)]
    pub capabilities: HashMap<String, Capability>,
    #[serde(default)]
    pub categories: Vec<Category>,
    pub sections: Vec<PrimerSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_strategy: Option<SelectionStrategy>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrimerMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_acp_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionStrategy {
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    #[serde(default)]
    pub weights: DimensionWeights,
    #[serde(default)]
    pub presets: HashMap<String, DimensionWeights>,
    #[serde(default)]
    pub phases: Vec<SelectionPhase>,
    #[serde(default = "default_min_budget")]
    pub minimum_budget: usize,
    #[serde(default = "default_true")]
    pub dynamic_modifiers_enabled: bool,
}

fn default_algorithm() -> String {
    "value-optimized".to_string()
}

fn default_min_budget() -> usize {
    80
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionPhase {
    pub name: String,
    #[serde(default)]
    pub filter: PhaseFilter,
    #[serde(default = "default_sort")]
    pub sort: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_percent: Option<f64>,
}

fn default_sort() -> String {
    "value-per-token".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PhaseFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_if: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safety_minimum: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

/// Parameters for primer generation
#[derive(Debug, Clone)]
pub struct GeneratePrimerRequest {
    /// Maximum token budget
    pub token_budget: usize,
    /// Output format
    pub format: OutputFormat,
    /// Preset weight configuration
    pub preset: Preset,
    /// Available capabilities
    pub capabilities: Vec<String>,
    /// Filter by categories
    pub categories: Option<Vec<String>>,
    /// Filter by tags
    pub tags: Option<Vec<String>>,
    /// Force include these section IDs
    pub force_include: Vec<String>,
}

impl Default for GeneratePrimerRequest {
    fn default() -> Self {
        Self {
            token_budget: 4000,
            format: OutputFormat::Markdown,
            preset: Preset::Balanced,
            capabilities: vec!["shell".to_string(), "file-read".to_string(), "file-write".to_string()],
            categories: None,
            tags: None,
            force_include: vec![],
        }
    }
}

/// Result of section selection
#[derive(Debug, Clone)]
pub struct SelectedSection {
    /// The section definition
    pub section: PrimerSection,
    /// Calculated weighted score
    pub score: f64,
    /// Actual token cost (after dynamic calculation)
    pub tokens: usize,
    /// Why this section was selected
    pub selection_reason: SelectionReason,
}

#[derive(Debug, Clone)]
pub enum SelectionReason {
    Required,
    ConditionallyRequired(String),
    SafetyCritical,
    ValueOptimized,
    ForcedInclude,
    Dependency(String),
}

/// Result of primer generation
#[derive(Debug, Clone)]
pub struct PrimerResult {
    /// Rendered primer content
    pub content: String,
    /// Sections included
    pub sections: Vec<SelectedSection>,
    /// Total tokens used
    pub tokens_used: usize,
    /// Token budget
    pub token_budget: usize,
    /// Sections excluded due to budget
    pub excluded_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_value_weighted_score() {
        let value = SectionValue {
            safety: 100,
            efficiency: 50,
            accuracy: 75,
            base: 60,
            modifiers: vec![],
        };

        let weights = DimensionWeights::default();
        let score = value.weighted_score(&weights);

        // safety: 100 * 1.5 = 150
        // efficiency: 50 * 1.0 = 50
        // accuracy: 75 * 1.0 = 75
        // base: 60 * 1.0 = 60
        // total: 335
        assert!((score - 335.0).abs() < 0.001);
    }

    #[test]
    fn test_preset_weights() {
        let safe = Preset::Safe.weights();
        assert!(safe.safety > safe.efficiency);

        let efficient = Preset::Efficient.weights();
        assert!(efficient.efficiency > efficient.safety);
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::from_str("markdown"), OutputFormat::Markdown);
        assert_eq!(OutputFormat::from_str("COMPACT"), OutputFormat::Compact);
        assert_eq!(OutputFormat::from_str("json"), OutputFormat::Json);
        assert_eq!(OutputFormat::from_str("unknown"), OutputFormat::Markdown);
    }
}
