//! @acp:module "Primer Scoring"
//! @acp:summary "Value calculation and condition evaluation for section selection"
//! @acp:domain daemon
//! @acp:layer service

use super::state::ProjectState;
use super::types::{DimensionWeights, ModifierDimension, PrimerSection, SectionValue, ValueModifier};

/// Scored section with all calculated values
#[derive(Debug, Clone)]
pub struct ScoredSection {
    /// Original section
    pub section: PrimerSection,
    /// Adjusted value after modifiers
    pub adjusted_value: SectionValue,
    /// Final weighted score
    pub weighted_score: f64,
    /// Value per token (for optimization)
    pub value_per_token: f64,
    /// Token count (resolved if dynamic)
    pub tokens: usize,
    /// Whether conditionally required (and condition met)
    pub is_conditionally_required: bool,
}

/// Score all sections with the given project state and weights
pub fn score_sections(
    sections: &[PrimerSection],
    state: &ProjectState,
    weights: &DimensionWeights,
    dynamic_modifiers_enabled: bool,
) -> Vec<ScoredSection> {
    sections
        .iter()
        .map(|section| score_section(section, state, weights, dynamic_modifiers_enabled))
        .collect()
}

/// Score a single section
pub fn score_section(
    section: &PrimerSection,
    state: &ProjectState,
    weights: &DimensionWeights,
    dynamic_modifiers_enabled: bool,
) -> ScoredSection {
    // Start with base value
    let mut adjusted = section.value.clone();

    // Apply modifiers if enabled
    if dynamic_modifiers_enabled {
        for modifier in &section.value.modifiers {
            if evaluate_condition(&modifier.condition, state) {
                apply_modifier(&mut adjusted, modifier);
            }
        }
    }

    // Calculate weighted score
    let weighted_score = adjusted.weighted_score(weights);

    // Resolve token count (for dynamic, estimate based on data)
    let tokens = resolve_token_count(section, state);

    // Calculate value per token
    let value_per_token = if tokens > 0 {
        weighted_score / tokens as f64
    } else {
        0.0
    };

    // Check if conditionally required
    let is_conditionally_required = section
        .required_if
        .as_ref()
        .map(|cond| evaluate_condition(cond, state))
        .unwrap_or(false);

    ScoredSection {
        section: section.clone(),
        adjusted_value: adjusted,
        weighted_score,
        value_per_token,
        tokens,
        is_conditionally_required,
    }
}

/// Evaluate a simple condition expression against project state
/// Supports: "path > N", "path >= N", "path < N", "path <= N", "path == N"
pub fn evaluate_condition(condition: &str, state: &ProjectState) -> bool {
    let condition = condition.trim();

    // Parse the condition
    let ops = [" >= ", " <= ", " > ", " < ", " == ", " != "];

    for op in ops {
        if let Some(idx) = condition.find(op) {
            let path = condition[..idx].trim();
            let value_str = condition[idx + op.len()..].trim();

            let Some(actual) = state.get_value(path) else {
                return false;
            };

            let Ok(expected) = value_str.parse::<f64>() else {
                return false;
            };

            return match op.trim() {
                ">=" => actual >= expected,
                "<=" => actual <= expected,
                ">" => actual > expected,
                "<" => actual < expected,
                "==" => (actual - expected).abs() < 0.001,
                "!=" => (actual - expected).abs() >= 0.001,
                _ => false,
            };
        }
    }

    // If no operator, treat as truthy check (value > 0)
    state.get_value(condition).map(|v| v > 0.0).unwrap_or(false)
}

/// Apply a modifier to adjusted value
fn apply_modifier(value: &mut SectionValue, modifier: &ValueModifier) {
    let apply_to_dimension = |v: &mut i32, modifier: &ValueModifier| {
        if let Some(add) = modifier.add {
            *v = (*v + add).clamp(0, 200); // Allow boosted values up to 200
        }
        if let Some(multiply) = modifier.multiply {
            *v = ((*v as f64) * multiply) as i32;
        }
        if let Some(set) = modifier.set {
            *v = set;
        }
    };

    match modifier.dimension {
        ModifierDimension::Safety => apply_to_dimension(&mut value.safety, modifier),
        ModifierDimension::Efficiency => apply_to_dimension(&mut value.efficiency, modifier),
        ModifierDimension::Accuracy => apply_to_dimension(&mut value.accuracy, modifier),
        ModifierDimension::Base => apply_to_dimension(&mut value.base, modifier),
        ModifierDimension::All => {
            apply_to_dimension(&mut value.safety, modifier);
            apply_to_dimension(&mut value.efficiency, modifier);
            apply_to_dimension(&mut value.accuracy, modifier);
            apply_to_dimension(&mut value.base, modifier);
        }
    }
}

/// Resolve token count for a section (handles dynamic sections)
fn resolve_token_count(section: &PrimerSection, state: &ProjectState) -> usize {
    match section.tokens.fixed_value() {
        Some(n) => n,
        None => {
            // Dynamic token count - estimate based on data source
            if let Some(ref data) = section.data {
                let item_count = estimate_item_count(&data.source, data.max_items, state);
                let item_tokens = data.item_tokens.unwrap_or(10);

                // Base tokens for header + item tokens
                let base = 15;
                base + (item_count * item_tokens)
            } else {
                30 // Default estimate
            }
        }
    }
}

/// Estimate item count for a data source
fn estimate_item_count(source: &str, max_items: Option<usize>, state: &ProjectState) -> usize {
    let estimated = match source {
        "cache.domains" => state.domains.count,
        "cache.layers" => state.layers.count,
        "cache.constraints.by_lock_level" => state.constraints.protected_count,
        "vars.variables" => state.variables.count,
        "attempts.active" => state.attempts.active_count,
        "cache.hacks" => state.hacks.count,
        "cache.entryPoints" => state.entry_points.count,
        _ => 5, // Default estimate
    };

    // Apply max_items limit
    max_items.map(|max| estimated.min(max)).unwrap_or(estimated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primer::types::TokenCount;

    fn create_test_state() -> ProjectState {
        ProjectState {
            constraints: crate::primer::state::ConstraintCounts {
                frozen_count: 5,
                restricted_count: 3,
                protected_count: 8,
                ..Default::default()
            },
            domains: crate::primer::state::DomainCounts {
                count: 4,
                names: vec!["auth".to_string(), "api".to_string()],
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_evaluate_condition_greater() {
        let state = create_test_state();

        assert!(evaluate_condition("constraints.frozenCount > 0", &state));
        assert!(!evaluate_condition("constraints.frozenCount > 10", &state));
        assert!(evaluate_condition("constraints.frozenCount >= 5", &state));
        assert!(!evaluate_condition("constraints.frozenCount >= 6", &state));
    }

    #[test]
    fn test_evaluate_condition_less() {
        let state = create_test_state();

        assert!(evaluate_condition("constraints.frozenCount < 10", &state));
        assert!(!evaluate_condition("constraints.frozenCount < 5", &state));
        assert!(evaluate_condition("constraints.frozenCount <= 5", &state));
    }

    #[test]
    fn test_evaluate_condition_equal() {
        let state = create_test_state();

        assert!(evaluate_condition("constraints.frozenCount == 5", &state));
        assert!(!evaluate_condition("constraints.frozenCount == 6", &state));
        assert!(evaluate_condition("constraints.frozenCount != 6", &state));
    }

    #[test]
    fn test_evaluate_condition_unknown_path() {
        let state = create_test_state();
        assert!(!evaluate_condition("unknown.path > 0", &state));
    }

    #[test]
    fn test_score_section_with_modifiers() {
        let state = create_test_state();
        let weights = DimensionWeights::default();

        let section = PrimerSection {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            category: "test".to_string(),
            priority: 1,
            tokens: TokenCount::Fixed(20),
            value: SectionValue {
                safety: 50,
                efficiency: 50,
                accuracy: 50,
                base: 50,
                modifiers: vec![ValueModifier {
                    condition: "constraints.frozenCount > 0".to_string(),
                    add: Some(30),
                    multiply: None,
                    set: None,
                    dimension: ModifierDimension::Safety,
                    reason: Some("Has frozen files".to_string()),
                }],
            },
            required: false,
            required_if: None,
            capabilities: vec![],
            capabilities_all: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            data: None,
            formats: Default::default(),
            tags: vec![],
        };

        let scored = score_section(&section, &state, &weights, true);

        // Safety should be boosted from 50 to 80
        assert_eq!(scored.adjusted_value.safety, 80);
        // Other dimensions unchanged
        assert_eq!(scored.adjusted_value.efficiency, 50);
    }

    #[test]
    fn test_score_section_modifier_not_applied() {
        let state = ProjectState::default(); // No frozen files
        let weights = DimensionWeights::default();

        let section = PrimerSection {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            category: "test".to_string(),
            priority: 1,
            tokens: TokenCount::Fixed(20),
            value: SectionValue {
                safety: 50,
                efficiency: 50,
                accuracy: 50,
                base: 50,
                modifiers: vec![ValueModifier {
                    condition: "constraints.frozenCount > 0".to_string(),
                    add: Some(30),
                    multiply: None,
                    set: None,
                    dimension: ModifierDimension::Safety,
                    reason: Some("Has frozen files".to_string()),
                }],
            },
            required: false,
            required_if: None,
            capabilities: vec![],
            capabilities_all: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            data: None,
            formats: Default::default(),
            tags: vec![],
        };

        let scored = score_section(&section, &state, &weights, true);

        // Modifier not applied - safety remains at 50
        assert_eq!(scored.adjusted_value.safety, 50);
    }
}
