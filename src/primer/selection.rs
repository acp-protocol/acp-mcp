//! @acp:module "Primer Selection"
//! @acp:summary "Section selection with budget optimization"
//! @acp:domain daemon
//! @acp:layer service

use std::collections::HashSet;

use super::scoring::ScoredSection;
use super::types::{GeneratePrimerRequest, SelectedSection, SelectionReason};

/// Selection result
#[derive(Debug)]
pub struct SelectionResult {
    /// Selected sections in inclusion order
    pub selected: Vec<SelectedSection>,
    /// Total tokens used
    pub tokens_used: usize,
    /// Sections excluded due to budget
    pub excluded_count: usize,
}

/// Select sections within budget using phase-based algorithm
///
/// Phase 1: Required sections (always include)
/// Phase 2: Conditionally required (based on project state)
/// Phase 3: Safety-critical sections (safety >= 80, up to 40% budget)
/// Phase 4: Value-optimized (remaining budget, sort by value-per-token)
pub fn select_sections(
    scored: &[ScoredSection],
    request: &GeneratePrimerRequest,
) -> SelectionResult {
    let mut selected: Vec<SelectedSection> = Vec::new();
    let mut tokens_used: usize = 0;
    let mut included_ids: HashSet<String> = HashSet::new();
    let mut excluded_ids: HashSet<String> = HashSet::new();

    let budget = request.token_budget;

    // Filter sections by capability
    let eligible: Vec<&ScoredSection> = scored
        .iter()
        .filter(|s| is_capability_compatible(s, &request.capabilities))
        .filter(|s| is_category_compatible(s, &request.categories))
        .filter(|s| is_tag_compatible(s, &request.tags))
        .collect();

    // Phase 1: Required sections (always include)
    let required: Vec<&ScoredSection> = eligible
        .iter()
        .filter(|s| s.section.required || request.force_include.contains(&s.section.id))
        .copied()
        .collect();

    for section in &required {
        if !can_include(section, &included_ids, &excluded_ids) {
            continue;
        }

        // Include dependencies first
        include_dependencies(
            section,
            &eligible,
            &mut selected,
            &mut included_ids,
            &mut excluded_ids,
            &mut tokens_used,
            budget,
        );

        // Include the section
        if tokens_used + section.tokens <= budget {
            selected.push(SelectedSection {
                section: section.section.clone(),
                score: section.weighted_score,
                tokens: section.tokens,
                selection_reason: if request.force_include.contains(&section.section.id) {
                    SelectionReason::ForcedInclude
                } else {
                    SelectionReason::Required
                },
            });
            tokens_used += section.tokens;
            included_ids.insert(section.section.id.clone());
            mark_conflicts(&section.section, &mut excluded_ids);
        }
    }

    // Phase 2: Conditionally required
    let conditionally_required: Vec<&ScoredSection> = eligible
        .iter()
        .filter(|s| s.is_conditionally_required && !included_ids.contains(&s.section.id))
        .copied()
        .collect();

    for section in &conditionally_required {
        if !can_include(section, &included_ids, &excluded_ids) {
            continue;
        }

        include_dependencies(
            section,
            &eligible,
            &mut selected,
            &mut included_ids,
            &mut excluded_ids,
            &mut tokens_used,
            budget,
        );

        if tokens_used + section.tokens <= budget {
            let reason = section
                .section
                .required_if
                .clone()
                .unwrap_or_else(|| "condition met".to_string());
            selected.push(SelectedSection {
                section: section.section.clone(),
                score: section.weighted_score,
                tokens: section.tokens,
                selection_reason: SelectionReason::ConditionallyRequired(reason),
            });
            tokens_used += section.tokens;
            included_ids.insert(section.section.id.clone());
            mark_conflicts(&section.section, &mut excluded_ids);
        }
    }

    // Phase 3: Safety-critical (safety >= 80, up to 40% of remaining budget)
    let safety_budget = ((budget - tokens_used) as f64 * 0.4) as usize;
    let mut safety_tokens = 0;

    let mut safety_critical: Vec<&ScoredSection> = eligible
        .iter()
        .filter(|s| {
            s.adjusted_value.safety >= 80
                && !included_ids.contains(&s.section.id)
                && !excluded_ids.contains(&s.section.id)
        })
        .copied()
        .collect();

    // Sort by safety score descending
    safety_critical.sort_by(|a, b| {
        b.adjusted_value
            .safety
            .cmp(&a.adjusted_value.safety)
            .then_with(|| b.weighted_score.partial_cmp(&a.weighted_score).unwrap())
    });

    for section in safety_critical {
        if safety_tokens >= safety_budget {
            break;
        }
        if !can_include(section, &included_ids, &excluded_ids) {
            continue;
        }
        if tokens_used + section.tokens > budget {
            continue;
        }

        include_dependencies(
            section,
            &eligible,
            &mut selected,
            &mut included_ids,
            &mut excluded_ids,
            &mut tokens_used,
            budget,
        );

        if tokens_used + section.tokens <= budget {
            selected.push(SelectedSection {
                section: section.section.clone(),
                score: section.weighted_score,
                tokens: section.tokens,
                selection_reason: SelectionReason::SafetyCritical,
            });
            tokens_used += section.tokens;
            safety_tokens += section.tokens;
            included_ids.insert(section.section.id.clone());
            mark_conflicts(&section.section, &mut excluded_ids);
        }
    }

    // Phase 4: Value-optimized (fill remaining budget)
    let mut value_optimized: Vec<&ScoredSection> = eligible
        .iter()
        .filter(|s| !included_ids.contains(&s.section.id) && !excluded_ids.contains(&s.section.id))
        .copied()
        .collect();

    // Sort by value per token descending
    value_optimized.sort_by(|a, b| {
        b.value_per_token
            .partial_cmp(&a.value_per_token)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for section in value_optimized {
        if tokens_used >= budget {
            break;
        }
        if !can_include(section, &included_ids, &excluded_ids) {
            continue;
        }
        if tokens_used + section.tokens > budget {
            continue;
        }

        include_dependencies(
            section,
            &eligible,
            &mut selected,
            &mut included_ids,
            &mut excluded_ids,
            &mut tokens_used,
            budget,
        );

        if tokens_used + section.tokens <= budget {
            selected.push(SelectedSection {
                section: section.section.clone(),
                score: section.weighted_score,
                tokens: section.tokens,
                selection_reason: SelectionReason::ValueOptimized,
            });
            tokens_used += section.tokens;
            included_ids.insert(section.section.id.clone());
            mark_conflicts(&section.section, &mut excluded_ids);
        }
    }

    // Count excluded
    let excluded_count = eligible.len() - selected.len();

    SelectionResult {
        selected,
        tokens_used,
        excluded_count,
    }
}

/// Check if a section can be included (not already included, not conflicted)
fn can_include(
    section: &ScoredSection,
    included: &HashSet<String>,
    excluded: &HashSet<String>,
) -> bool {
    !included.contains(&section.section.id) && !excluded.contains(&section.section.id)
}

/// Mark conflicting sections as excluded
fn mark_conflicts(
    section: &super::types::PrimerSection,
    excluded: &mut HashSet<String>,
) {
    for conflict in &section.conflicts_with {
        excluded.insert(conflict.clone());
    }
}

/// Check if section is compatible with available capabilities
fn is_capability_compatible(section: &ScoredSection, capabilities: &[String]) -> bool {
    // If section requires all capabilities, check all
    if !section.section.capabilities_all.is_empty() {
        return section
            .section
            .capabilities_all
            .iter()
            .all(|c| capabilities.contains(c));
    }

    // If section requires any capability, check any
    if !section.section.capabilities.is_empty() {
        return section
            .section
            .capabilities
            .iter()
            .any(|c| capabilities.contains(c));
    }

    // No capability requirements
    true
}

/// Check if section is compatible with category filter
fn is_category_compatible(section: &ScoredSection, categories: &Option<Vec<String>>) -> bool {
    match categories {
        Some(cats) => cats.contains(&section.section.category),
        None => true,
    }
}

/// Check if section is compatible with tag filter
fn is_tag_compatible(section: &ScoredSection, tags: &Option<Vec<String>>) -> bool {
    match tags {
        Some(filter_tags) => section
            .section
            .tags
            .iter()
            .any(|t| filter_tags.contains(t)),
        None => true,
    }
}

/// Include dependencies recursively
fn include_dependencies(
    section: &ScoredSection,
    all_sections: &[&ScoredSection],
    selected: &mut Vec<SelectedSection>,
    included: &mut HashSet<String>,
    excluded: &mut HashSet<String>,
    tokens_used: &mut usize,
    budget: usize,
) {
    for dep_id in &section.section.depends_on {
        if included.contains(dep_id) {
            continue;
        }

        // Find the dependency section
        if let Some(dep) = all_sections.iter().find(|s| &s.section.id == dep_id) {
            if excluded.contains(&dep.section.id) {
                continue;
            }

            // Recursively include its dependencies first
            include_dependencies(dep, all_sections, selected, included, excluded, tokens_used, budget);

            // Include the dependency
            if *tokens_used + dep.tokens <= budget {
                selected.push(SelectedSection {
                    section: dep.section.clone(),
                    score: dep.weighted_score,
                    tokens: dep.tokens,
                    selection_reason: SelectionReason::Dependency(section.section.id.clone()),
                });
                *tokens_used += dep.tokens;
                included.insert(dep.section.id.clone());
                mark_conflicts(&dep.section, excluded);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primer::types::{
        DimensionWeights, OutputFormat, Preset, SectionFormats, SectionValue, TokenCount,
    };

    fn create_test_section(
        id: &str,
        tokens: usize,
        safety: i32,
        required: bool,
    ) -> ScoredSection {
        let section = super::super::types::PrimerSection {
            id: id.to_string(),
            name: id.to_string(),
            description: None,
            category: "test".to_string(),
            priority: 1,
            tokens: TokenCount::Fixed(tokens),
            value: SectionValue {
                safety,
                efficiency: 50,
                accuracy: 50,
                base: 50,
                modifiers: vec![],
            },
            required,
            required_if: None,
            capabilities: vec![],
            capabilities_all: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            data: None,
            formats: SectionFormats::default(),
            tags: vec![],
        };

        let weights = DimensionWeights::default();
        let weighted_score = section.value.weighted_score(&weights);

        ScoredSection {
            section,
            adjusted_value: SectionValue {
                safety,
                efficiency: 50,
                accuracy: 50,
                base: 50,
                modifiers: vec![],
            },
            weighted_score,
            value_per_token: weighted_score / tokens as f64,
            tokens,
            is_conditionally_required: false,
        }
    }

    #[test]
    fn test_select_required_first() {
        let sections = vec![
            create_test_section("optional", 100, 50, false),
            create_test_section("required", 50, 50, true),
        ];

        let request = GeneratePrimerRequest {
            token_budget: 200,
            format: OutputFormat::Markdown,
            preset: Preset::Balanced,
            capabilities: vec![],
            categories: None,
            tags: None,
            force_include: vec![],
        };

        let result = select_sections(&sections, &request);

        // Required section should be included first
        assert!(result
            .selected
            .iter()
            .any(|s| s.section.id == "required"));
        assert!(matches!(
            result.selected[0].selection_reason,
            SelectionReason::Required
        ));
    }

    #[test]
    fn test_select_within_budget() {
        let sections = vec![
            create_test_section("a", 100, 50, false),
            create_test_section("b", 100, 50, false),
            create_test_section("c", 100, 50, false),
        ];

        let request = GeneratePrimerRequest {
            token_budget: 150,
            format: OutputFormat::Markdown,
            preset: Preset::Balanced,
            capabilities: vec![],
            categories: None,
            tags: None,
            force_include: vec![],
        };

        let result = select_sections(&sections, &request);

        // Should only include 1 section within budget
        assert_eq!(result.selected.len(), 1);
        assert!(result.tokens_used <= 150);
    }

    #[test]
    fn test_safety_critical_prioritized() {
        let sections = vec![
            create_test_section("low_safety", 50, 30, false),
            create_test_section("high_safety", 50, 90, false),
        ];

        let request = GeneratePrimerRequest {
            token_budget: 100,
            format: OutputFormat::Markdown,
            preset: Preset::Balanced,
            capabilities: vec![],
            categories: None,
            tags: None,
            force_include: vec![],
        };

        let result = select_sections(&sections, &request);

        // High safety section should be selected
        assert!(result
            .selected
            .iter()
            .any(|s| s.section.id == "high_safety"));
    }
}
