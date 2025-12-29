//! @acp:module "Primer Rendering"
//! @acp:summary "Template rendering for primer output"
//! @acp:domain daemon
//! @acp:layer service

use acp::cache::Cache;
use handlebars::Handlebars;
use serde_json::{json, Value};
use std::collections::HashMap;

use super::types::{FormatTemplate, OutputFormat, PrimerSection, SelectedSection};

/// Renderer for primer sections
pub struct PrimerRenderer<'a> {
    handlebars: Handlebars<'a>,
    format: OutputFormat,
}

impl<'a> PrimerRenderer<'a> {
    pub fn new(format: OutputFormat) -> Self {
        let mut handlebars = Handlebars::new();
        // Don't escape HTML entities
        handlebars.register_escape_fn(handlebars::no_escape);

        Self { handlebars, format }
    }

    /// Render all selected sections
    pub fn render(
        &self,
        sections: &[SelectedSection],
        cache: &Cache,
    ) -> Result<String, RenderError> {
        let separator = match self.format {
            OutputFormat::Markdown => "\n\n",
            OutputFormat::Compact => " | ",
            OutputFormat::Json => ",\n",
        };

        let rendered: Vec<String> = sections
            .iter()
            .filter_map(|s| self.render_section(&s.section, cache).ok())
            .filter(|s| !s.is_empty())
            .collect();

        if self.format == OutputFormat::Json {
            Ok(format!("[\n{}\n]", rendered.join(separator)))
        } else {
            Ok(rendered.join(separator))
        }
    }

    /// Render a single section
    pub fn render_section(
        &self,
        section: &PrimerSection,
        cache: &Cache,
    ) -> Result<String, RenderError> {
        let template = section
            .formats
            .get(self.format)
            .ok_or(RenderError::MissingFormat(self.format))?;

        // Check if this is a dynamic section with data
        if let Some(ref data_config) = section.data {
            self.render_dynamic_section(section, template, data_config, cache)
        } else {
            self.render_static_section(template)
        }
    }

    /// Render a static section (simple template)
    fn render_static_section(&self, template: &FormatTemplate) -> Result<String, RenderError> {
        if let Some(ref tpl) = template.template {
            // For simple templates, just return the template string
            // (no variable substitution needed for static content)
            Ok(tpl.clone())
        } else {
            Ok(String::new())
        }
    }

    /// Render a dynamic section with data
    fn render_dynamic_section(
        &self,
        section: &PrimerSection,
        template: &FormatTemplate,
        data_config: &super::types::SectionData,
        cache: &Cache,
    ) -> Result<String, RenderError> {
        // Extract data from cache
        let items = self.extract_data(&data_config.source, data_config, cache);

        if items.is_empty() {
            return match data_config.empty_behavior {
                super::types::EmptyBehavior::Exclude => Ok(String::new()),
                super::types::EmptyBehavior::Placeholder => {
                    Ok(template.empty_template.clone().unwrap_or_default())
                }
                super::types::EmptyBehavior::Error => {
                    Err(RenderError::EmptyData(section.id.clone()))
                }
            };
        }

        // Render items
        let mut rendered_items: Vec<String> = Vec::new();

        if let Some(ref item_tpl) = template.item_template {
            for item in &items {
                let rendered = self.render_template(item_tpl, item)?;
                rendered_items.push(rendered);
            }
        }

        // Build final output
        let mut output = String::new();

        if let Some(ref header) = template.header {
            output.push_str(header);
        }

        output.push_str(&rendered_items.join(&template.separator));

        if let Some(ref footer) = template.footer {
            output.push_str(footer);
        }

        Ok(output)
    }

    /// Render a handlebars template with data
    fn render_template(&self, template: &str, data: &Value) -> Result<String, RenderError> {
        self.handlebars
            .render_template(template, data)
            .map_err(|e| RenderError::Template(e.to_string()))
    }

    /// Extract data from cache based on source path
    fn extract_data(
        &self,
        source: &str,
        config: &super::types::SectionData,
        cache: &Cache,
    ) -> Vec<Value> {
        let mut items: Vec<Value> = match source {
            "cache.domains" => self.extract_domains(cache, config),
            "cache.constraints.by_lock_level" => self.extract_constraints(cache, config),
            "cache.layers" => self.extract_layers(cache),
            "cache.entryPoints" => self.extract_entry_points(cache),
            _ => Vec::new(),
        };

        // Apply sorting
        if let Some(ref sort_by) = config.sort_by {
            items.sort_by(|a, b| {
                let a_val = a.get(sort_by);
                let b_val = b.get(sort_by);

                match (a_val, b_val) {
                    (Some(Value::Number(a)), Some(Value::Number(b))) => {
                        let a_f = a.as_f64().unwrap_or(0.0);
                        let b_f = b.as_f64().unwrap_or(0.0);
                        match config.sort_order {
                            super::types::SortOrder::Asc => {
                                a_f.partial_cmp(&b_f).unwrap_or(std::cmp::Ordering::Equal)
                            }
                            super::types::SortOrder::Desc => {
                                b_f.partial_cmp(&a_f).unwrap_or(std::cmp::Ordering::Equal)
                            }
                        }
                    }
                    (Some(Value::String(a)), Some(Value::String(b))) => match config.sort_order {
                        super::types::SortOrder::Asc => a.cmp(b),
                        super::types::SortOrder::Desc => b.cmp(a),
                    },
                    _ => std::cmp::Ordering::Equal,
                }
            });
        }

        // Apply max_items limit
        if let Some(max) = config.max_items {
            items.truncate(max);
        }

        items
    }

    /// Extract domains from cache
    fn extract_domains(&self, cache: &Cache, _config: &super::types::SectionData) -> Vec<Value> {
        cache
            .domains
            .iter()
            .map(|(name, domain)| {
                let mut obj = serde_json::Map::new();
                obj.insert("name".to_string(), json!(name));
                obj.insert("fileCount".to_string(), json!(domain.files.len()));
                if let Some(ref desc) = domain.description {
                    obj.insert("description".to_string(), json!(desc));
                }
                Value::Object(obj)
            })
            .collect()
    }

    /// Extract constraints (protected files) from cache
    fn extract_constraints(&self, cache: &Cache, config: &super::types::SectionData) -> Vec<Value> {
        use acp::constraints::LockLevel;

        let Some(ref constraints) = cache.constraints else {
            return Vec::new();
        };

        // Get the filter levels
        let filter_levels: Vec<&str> = match &config.filter {
            Some(super::types::DataFilter::Include(levels)) => {
                levels.iter().map(|s| s.as_str()).collect()
            }
            _ => vec!["frozen", "restricted"],
        };

        constraints
            .by_file
            .iter()
            .filter(|(_, c)| {
                c.mutation
                    .as_ref()
                    .map(|m| {
                        let level_str = match m.level {
                            LockLevel::Frozen => "frozen",
                            LockLevel::Restricted => "restricted",
                            LockLevel::ApprovalRequired => "approval-required",
                            LockLevel::TestsRequired => "tests-required",
                            LockLevel::DocsRequired => "docs-required",
                            _ => "normal",
                        };
                        filter_levels.contains(&level_str)
                    })
                    .unwrap_or(false)
            })
            .map(|(path, c)| {
                let mut obj = serde_json::Map::new();
                obj.insert("path".to_string(), json!(path));
                if let Some(ref mutation) = c.mutation {
                    let level_str = match mutation.level {
                        LockLevel::Frozen => "frozen",
                        LockLevel::Restricted => "restricted",
                        LockLevel::ApprovalRequired => "approval-required",
                        LockLevel::TestsRequired => "tests-required",
                        LockLevel::DocsRequired => "docs-required",
                        _ => "normal",
                    };
                    obj.insert("level".to_string(), json!(level_str));
                    if let Some(ref reason) = mutation.reason {
                        obj.insert("reason".to_string(), json!(reason));
                    }
                }
                Value::Object(obj)
            })
            .collect()
    }

    /// Extract layers from cache
    fn extract_layers(&self, cache: &Cache) -> Vec<Value> {
        // Count files per layer
        let mut layer_counts: HashMap<String, usize> = HashMap::new();

        for file in cache.files.values() {
            if let Some(ref layer) = file.layer {
                *layer_counts.entry(layer.clone()).or_default() += 1;
            }
        }

        layer_counts
            .into_iter()
            .map(|(name, count)| {
                let mut obj = serde_json::Map::new();
                obj.insert("name".to_string(), json!(name));
                obj.insert("fileCount".to_string(), json!(count));
                Value::Object(obj)
            })
            .collect()
    }

    /// Extract entry points from cache
    fn extract_entry_points(&self, cache: &Cache) -> Vec<Value> {
        // Look for common entry point patterns
        let entry_patterns = [
            "main.rs", "main.ts", "main.py", "index.ts", "index.js", "app.ts", "app.py", "mod.rs",
        ];

        cache
            .files
            .values()
            .filter(|f| {
                let path = f.path.to_lowercase();
                entry_patterns
                    .iter()
                    .any(|p| path.ends_with(p) || path.contains("/src/") && path.ends_with(".rs"))
            })
            .take(10)
            .map(|f| {
                let mut obj = serde_json::Map::new();
                obj.insert("path".to_string(), json!(f.path));
                obj.insert("type".to_string(), json!(format!("{:?}", f.language)));
                Value::Object(obj)
            })
            .collect()
    }
}

/// Render error types
#[derive(Debug)]
pub enum RenderError {
    MissingFormat(OutputFormat),
    Template(String),
    EmptyData(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFormat(format) => write!(f, "Missing format template: {:?}", format),
            Self::Template(msg) => write!(f, "Template error: {}", msg),
            Self::EmptyData(section) => write!(f, "Empty data for section: {}", section),
        }
    }
}

impl std::error::Error for RenderError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primer::types::{FormatTemplate, SectionFormats, SectionValue, TokenCount};

    fn create_test_section() -> PrimerSection {
        PrimerSection {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            category: "test".to_string(),
            priority: 1,
            tokens: TokenCount::Fixed(20),
            value: SectionValue::default(),
            required: false,
            required_if: None,
            capabilities: vec![],
            capabilities_all: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            data: None,
            formats: SectionFormats {
                markdown: Some(FormatTemplate {
                    template: Some("This is a test section.".to_string()),
                    header: None,
                    footer: None,
                    item_template: None,
                    separator: "\n".to_string(),
                    empty_template: None,
                }),
                compact: Some(FormatTemplate {
                    template: Some("Test section".to_string()),
                    header: None,
                    footer: None,
                    item_template: None,
                    separator: " ".to_string(),
                    empty_template: None,
                }),
                json: None,
            },
            tags: vec![],
        }
    }

    #[test]
    fn test_render_static_section() {
        let renderer = PrimerRenderer::new(OutputFormat::Markdown);
        let cache = Cache::new("test", ".");
        let section = create_test_section();

        let result = renderer.render_section(&section, &cache);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "This is a test section.");
    }

    #[test]
    fn test_render_compact_format() {
        let renderer = PrimerRenderer::new(OutputFormat::Compact);
        let cache = Cache::new("test", ".");
        let section = create_test_section();

        let result = renderer.render_section(&section, &cache);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test section");
    }

    #[test]
    fn test_handlebars_template() {
        let renderer = PrimerRenderer::new(OutputFormat::Markdown);
        let data = json!({
            "name": "test-domain",
            "fileCount": 42
        });

        let result = renderer.render_template("**{{name}}** ({{fileCount}} files)", &data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "**test-domain** (42 files)");
    }
}
