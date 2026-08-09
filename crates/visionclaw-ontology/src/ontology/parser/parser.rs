use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Regex for matching markdown headings (e.g., "# Title")
/// Compiled once at startup - pattern is a compile-time constant that is always valid
static HEADING_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^#\s+(.+)$").expect("HEADING_RE pattern is a compile-time constant"));

/// Regex for matching Logseq property syntax (e.g., "key:: value" or "owl:class:: value")
/// Supports both inline properties and Logseq outline format with "- " prefix
/// Compiled once at startup - pattern is a compile-time constant that is always valid
static PROPERTY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?:-\s+)?([a-zA-Z][a-zA-Z0-9:_-]*)::\s*(.+)$")
        .expect("PROPERTY_RE pattern is a compile-time constant")
});

#[derive(Debug, Clone)]
pub struct LogseqPage {
    pub title: String,
    pub properties: HashMap<String, Vec<String>>,
    pub owl_blocks: Vec<String>,
}

pub fn parse_logseq_file(path: &Path) -> Result<LogseqPage> {
    let content =
        fs::read_to_string(path).context(format!("Failed to read file: {}", path.display()))?;

    let title = extract_title(path, &content);
    let properties = extract_properties(&content);
    let owl_blocks = extract_owl_blocks(&content)?;

    Ok(LogseqPage {
        title,
        properties,
        owl_blocks,
    })
}

fn extract_title(path: &Path, content: &str) -> String {
    // Check for markdown heading first
    for line in content.lines() {
        if let Some(cap) = HEADING_RE.captures(line) {
            return cap[1].trim().to_string();
        }
    }

    // Fall back to filename (without extension) as title
    path.file_stem()
        .and_then(|s| s.to_str())
        // file_stem returns None only for paths like ".." or ending in ".."
        // which is extremely rare for actual files; "Untitled" is a safe default
        .unwrap_or("Untitled")
        .to_string()
}

// TODO: Optimize to single-pass state machine for better performance
// Current multi-pass approach is acceptable for typical ontology sizes
fn extract_properties(content: &str) -> HashMap<String, Vec<String>> {
    let mut properties = HashMap::new();

    for line in content.lines() {
        if let Some(cap) = PROPERTY_RE.captures(line.trim()) {
            let key = cap[1].to_string();
            let value = cap[2].to_string();

            // Split comma-separated values
            let values: Vec<String> = value
                .split(',')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect();

            properties
                .entry(key)
                .or_insert_with(Vec::new)
                .extend(values);
        }
    }

    properties
}

fn extract_owl_blocks(content: &str) -> Result<Vec<String>> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        let fence_match = if line.starts_with("```") {
            Some(line)
        } else if line.starts_with("- ```") {
            Some(&line[2..])
        } else {
            None
        };

        if let Some(fence_line) = fence_match {
            let language = fence_line.trim_start_matches("```").trim();

            if language == "clojure" || language.is_empty() {
                i += 1;
                if i >= lines.len() {
                    break;
                }

                let should_extract = if language == "clojure" {
                    true
                } else if lines[i].trim().starts_with("owl:functional-syntax::") {
                    i += 1;
                    true
                } else {
                    false
                };

                if should_extract {
                    let mut block_lines = Vec::new();
                    while i < lines.len() {
                        let current_line = lines[i];
                        if current_line.trim().starts_with("```") {
                            break;
                        }

                        let trimmed = current_line.trim_start();
                        if !trimmed.is_empty()
                            && !trimmed.starts_with(";;")
                            && !trimmed.starts_with("#")
                            && trimmed != "|"
                        {
                            block_lines.push(trimmed);
                        }
                        i += 1;
                    }

                    let block_text = block_lines.join("\n");
                    let is_owl = block_text.contains("Declaration(")
                        || block_text.contains("SubClassOf(")
                        || block_text.contains("EquivalentClasses(")
                        || block_text.contains("DisjointClasses(")
                        || block_text.contains("ObjectProperty(")
                        || block_text.contains("DataProperty(");

                    if is_owl && !block_lines.is_empty() {
                        blocks.push(block_text);
                    }
                }
            }
            i += 1;
            continue;
        }

        if line.starts_with("owl:functional-syntax::") {
            // Check if | is on the same line or the next line
            let remainder = line.trim_start_matches("owl:functional-syntax::").trim();

            if remainder == "|" {
                // | is on the same line, move to next line for content
                i += 1;
            } else if remainder.is_empty() {
                // Check if next line starts with |
                i += 1;
                if i >= lines.len() {
                    break;
                }

                if !lines[i].trim().starts_with('|') {
                    i += 1;
                    continue;
                }

                i += 1;
            } else {
                // Invalid format
                i += 1;
                continue;
            }

            if i >= lines.len() {
                break;
            }

            let mut block_lines = Vec::new();
            let base_indent = if i < lines.len() {
                lines[i].len() - lines[i].trim_start().len()
            } else {
                0
            };

            while i < lines.len() {
                let current_line = lines[i];
                let current_indent = current_line.len() - current_line.trim_start().len();

                if !current_line.trim().is_empty() && current_indent < base_indent {
                    break;
                }

                if current_line.trim_start().starts_with('#')
                    || current_line.trim().starts_with("```")
                    || (current_line.contains("::") && !current_line.trim().starts_with("|"))
                {
                    break;
                }

                if current_indent >= base_indent && !current_line.trim().is_empty() {
                    let trimmed = if current_indent >= base_indent {
                        &current_line[base_indent..]
                    } else {
                        current_line.trim_start()
                    };
                    block_lines.push(trimmed);
                }

                i += 1;
            }

            if !block_lines.is_empty() {
                blocks.push(block_lines.join("\n"));
            }
        } else {
            i += 1;
        }
    }

    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_properties() {
        let content = r#"
# Test Page

term-id:: 20067
maturity:: mature
has-part:: [[Visual Mesh]], [[Animation Rig]]
"#;

        let props = extract_properties(content);
        assert_eq!(
            props.get("term-id").expect("Missing required key: term-id")[0],
            "20067"
        );
        assert_eq!(
            props
                .get("maturity")
                .expect("Missing required key: maturity")[0],
            "mature"
        );
        assert_eq!(
            props
                .get("has-part")
                .expect("Missing required key: has-part")
                .len(),
            2
        );
    }

    #[test]
    fn test_extract_owl_blocks() {
        let content = r#"
owl:functional-syntax:: |
  Declaration(Class(mv:Avatar))
  SubClassOf(mv:Avatar mv:VirtualEntity)
"#;

        let blocks = extract_owl_blocks(content).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("Declaration(Class(mv:Avatar))"));
    }

    #[test]
    fn test_extract_owl_blocks_code_fence() {
        let content = r#"
	- ## OWL Axioms
	  collapsed:: true
		- ```
		  owl:functional-syntax:: |
		    Declaration(Class(mv:Avatar))

		    # Classification
		    SubClassOf(mv:Avatar mv:VirtualEntity)
		    SubClassOf(mv:Avatar mv:Agent)
		  ```
"#;

        let blocks = extract_owl_blocks(content).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("Declaration(Class(mv:Avatar))"));
        assert!(blocks[0].contains("SubClassOf(mv:Avatar mv:VirtualEntity)"));
        assert!(blocks[0].contains("SubClassOf(mv:Avatar mv:Agent)"));
    }

    #[test]
    fn test_extract_properties_from_outline() {
        let content = r#"
- OntologyBlock
  collapsed:: true
	- term-id:: 20067
	- preferred-term:: Avatar
	- owl:class:: mv:Avatar
	- owl:physicality:: VirtualEntity
	- owl:role:: Agent
"#;

        let props = extract_properties(content);
        assert_eq!(
            props.get("term-id").expect("Missing required key: term-id")[0],
            "20067"
        );
        assert_eq!(
            props
                .get("preferred-term")
                .expect("Missing required key: preferred-term")[0],
            "Avatar"
        );
        assert_eq!(
            props
                .get("owl:class")
                .expect("Missing required key: owl:class")[0],
            "mv:Avatar"
        );
        assert_eq!(
            props
                .get("owl:physicality")
                .expect("Missing required key: owl:physicality")[0],
            "VirtualEntity"
        );
        assert_eq!(
            props
                .get("owl:role")
                .expect("Missing required key: owl:role")[0],
            "Agent"
        );
    }
}
