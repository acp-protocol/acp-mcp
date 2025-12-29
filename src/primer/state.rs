//! @acp:module "Primer State"
//! @acp:summary "Project state extraction from cache for condition evaluation"
//! @acp:domain daemon
//! @acp:layer service

use acp::cache::Cache;
use serde::Serialize;
use std::collections::HashMap;

/// Project state extracted from cache for condition evaluation
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectState {
    pub constraints: ConstraintCounts,
    pub domains: DomainCounts,
    pub layers: LayerCounts,
    pub variables: VariableCounts,
    pub attempts: AttemptCounts,
    pub hacks: HackCounts,
    pub entry_points: EntryPointCounts,
    pub stats: ProjectStats,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ConstraintCounts {
    pub frozen_count: usize,
    pub restricted_count: usize,
    pub approval_count: usize,
    pub tests_required_count: usize,
    pub docs_required_count: usize,
    pub protected_count: usize,
    pub total_count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DomainCounts {
    pub count: usize,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LayerCounts {
    pub count: usize,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct VariableCounts {
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct AttemptCounts {
    pub active_count: usize,
    pub total_count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct HackCounts {
    pub count: usize,
    pub expired_count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct EntryPointCounts {
    pub count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProjectStats {
    pub file_count: usize,
    pub symbol_count: usize,
    pub line_count: usize,
    pub annotation_coverage: f64,
}

impl ProjectState {
    /// Build project state from cache
    pub fn from_cache(cache: &Cache) -> Self {
        let constraints = Self::extract_constraints(cache);
        let domains = Self::extract_domains(cache);
        let layers = Self::extract_layers(cache);

        Self {
            constraints,
            domains,
            layers,
            variables: VariableCounts::default(), // Filled from vars file separately
            attempts: AttemptCounts::default(),   // Filled from attempts file separately
            hacks: HackCounts::default(),         // TODO: extract from cache if we track hacks
            entry_points: EntryPointCounts::default(), // TODO: extract entry points
            stats: ProjectStats {
                file_count: cache.files.len(),
                symbol_count: cache.symbols.len(),
                line_count: cache.stats.lines,
                annotation_coverage: cache.stats.annotation_coverage,
            },
        }
    }

    fn extract_constraints(cache: &Cache) -> ConstraintCounts {
        use acp::constraints::LockLevel;

        let Some(ref constraints) = cache.constraints else {
            return ConstraintCounts::default();
        };

        let mut counts = ConstraintCounts::default();

        for file_constraint in constraints.by_file.values() {
            counts.total_count += 1;

            // Check mutation constraint for lock level
            if let Some(ref mutation) = file_constraint.mutation {
                match mutation.level {
                    LockLevel::Frozen => {
                        counts.frozen_count += 1;
                        counts.protected_count += 1;
                    }
                    LockLevel::Restricted => {
                        counts.restricted_count += 1;
                        counts.protected_count += 1;
                    }
                    LockLevel::ApprovalRequired => counts.approval_count += 1,
                    LockLevel::TestsRequired => counts.tests_required_count += 1,
                    LockLevel::DocsRequired => counts.docs_required_count += 1,
                    _ => {}
                }
            }
        }

        counts
    }

    fn extract_domains(cache: &Cache) -> DomainCounts {
        DomainCounts {
            count: cache.domains.len(),
            names: cache.domains.keys().cloned().collect(),
        }
    }

    fn extract_layers(cache: &Cache) -> LayerCounts {
        // Extract unique layers from files
        let mut layers: HashMap<String, usize> = HashMap::new();

        for file in cache.files.values() {
            if let Some(ref layer) = file.layer {
                *layers.entry(layer.clone()).or_default() += 1;
            }
        }

        LayerCounts {
            count: layers.len(),
            names: layers.keys().cloned().collect(),
        }
    }

    /// Set variable counts from vars file
    #[allow(dead_code)]
    pub fn with_variable_count(mut self, count: usize) -> Self {
        self.variables.count = count;
        self
    }

    /// Set attempt counts
    #[allow(dead_code)]
    pub fn with_attempts(mut self, active: usize, total: usize) -> Self {
        self.attempts.active_count = active;
        self.attempts.total_count = total;
        self
    }

    /// Get a value by path for condition evaluation
    /// Supports paths like "constraints.frozenCount", "domains.count", etc.
    pub fn get_value(&self, path: &str) -> Option<f64> {
        let parts: Vec<&str> = path.split('.').collect();

        match parts.as_slice() {
            ["constraints", "frozenCount"] => Some(self.constraints.frozen_count as f64),
            ["constraints", "restrictedCount"] => Some(self.constraints.restricted_count as f64),
            ["constraints", "approvalCount"] => Some(self.constraints.approval_count as f64),
            ["constraints", "testsRequiredCount"] => {
                Some(self.constraints.tests_required_count as f64)
            }
            ["constraints", "docsRequiredCount"] => {
                Some(self.constraints.docs_required_count as f64)
            }
            ["constraints", "protectedCount"] => Some(self.constraints.protected_count as f64),
            ["constraints", "totalCount"] => Some(self.constraints.total_count as f64),
            ["domains", "count"] => Some(self.domains.count as f64),
            ["layers", "count"] => Some(self.layers.count as f64),
            ["variables", "count"] => Some(self.variables.count as f64),
            ["attempts", "activeCount"] => Some(self.attempts.active_count as f64),
            ["attempts", "totalCount"] => Some(self.attempts.total_count as f64),
            ["hacks", "count"] => Some(self.hacks.count as f64),
            ["hacks", "expiredCount"] => Some(self.hacks.expired_count as f64),
            ["entryPoints", "count"] => Some(self.entry_points.count as f64),
            ["stats", "fileCount"] => Some(self.stats.file_count as f64),
            ["stats", "symbolCount"] => Some(self.stats.symbol_count as f64),
            ["stats", "lineCount"] => Some(self.stats.line_count as f64),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_value() {
        let state = ProjectState {
            constraints: ConstraintCounts {
                frozen_count: 5,
                restricted_count: 3,
                protected_count: 8,
                ..Default::default()
            },
            domains: DomainCounts {
                count: 4,
                names: vec!["auth".to_string(), "api".to_string()],
            },
            ..Default::default()
        };

        assert_eq!(state.get_value("constraints.frozenCount"), Some(5.0));
        assert_eq!(state.get_value("constraints.protectedCount"), Some(8.0));
        assert_eq!(state.get_value("domains.count"), Some(4.0));
        assert_eq!(state.get_value("unknown.path"), None);
    }
}
