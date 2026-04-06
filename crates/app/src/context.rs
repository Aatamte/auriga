use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Layer 0 — the project map. Always injected on agent spawn.
#[derive(Debug, Clone, Default)]
pub struct ContextMap {
    pub content: String,
    pub last_verified: Option<String>,
}

/// Layer 1 — per-file annotation.
#[derive(Debug, Clone, Default)]
pub struct FileAnnotation {
    pub purpose: String,
    pub key_types: Vec<(String, String)>,
    pub invariants: Vec<String>,
    pub hazards: Vec<String>,
    pub examples: Vec<String>,
}

/// Layer 2 — a deep context document.
#[derive(Debug, Clone)]
pub struct DeepContext {
    pub name: String,
    pub path: PathBuf,
    pub content: String,
    pub last_verified: Option<String>,
}

/// Everything loaded from .agent-orchestrator/context/.
#[derive(Debug, Clone, Default)]
pub struct ContextStore {
    pub map: ContextMap,
    pub annotations: BTreeMap<String, FileAnnotation>,
    pub deep_contexts: Vec<DeepContext>,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

fn context_dir() -> PathBuf {
    crate::config::dir_path().join("context")
}

/// Load the full context store from disk.
pub fn load() -> ContextStore {
    let dir = context_dir();
    ContextStore {
        map: load_map(&dir),
        annotations: load_annotations(&dir),
        deep_contexts: load_deep_contexts(&dir),
    }
}

/// Load just the Layer 0 map content (for injection into system prompts).
pub fn load_map_content() -> String {
    let dir = context_dir();
    let map = load_map(&dir);
    map.content
}

/// Get the annotation for a specific file path, if one exists.
pub fn annotation_for<'a>(store: &'a ContextStore, path: &str) -> Option<&'a FileAnnotation> {
    store.annotations.get(path)
}

/// Format an annotation as text suitable for prepending to agent context.
pub fn format_annotation(path: &str, ann: &FileAnnotation) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Context: {}\n", path));
    if !ann.purpose.is_empty() {
        out.push_str(&format!("Purpose: {}\n", ann.purpose));
    }
    if !ann.key_types.is_empty() {
        out.push_str("Key types:\n");
        for (name, desc) in &ann.key_types {
            out.push_str(&format!("  - {}: {}\n", name, desc));
        }
    }
    if !ann.invariants.is_empty() {
        out.push_str("Invariants:\n");
        for inv in &ann.invariants {
            out.push_str(&format!("  - {}\n", inv));
        }
    }
    if !ann.hazards.is_empty() {
        out.push_str("Hazards:\n");
        for h in &ann.hazards {
            out.push_str(&format!("  - {}\n", h));
        }
    }
    if !ann.examples.is_empty() {
        out.push_str("Examples:\n");
        for e in &ann.examples {
            out.push_str(&format!("  - {}\n", e));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Internal loaders
// ---------------------------------------------------------------------------

fn load_map(dir: &Path) -> ContextMap {
    let path = dir.join("map.md");
    let Ok(raw) = fs::read_to_string(&path) else {
        return ContextMap::default();
    };
    let (frontmatter, body) = split_frontmatter(&raw);
    let last_verified = extract_field(&frontmatter, "last_verified");
    ContextMap {
        content: body.trim().to_string(),
        last_verified,
    }
}

fn load_annotations(dir: &Path) -> BTreeMap<String, FileAnnotation> {
    let path = dir.join("annotations.yaml");
    let Ok(raw) = fs::read_to_string(&path) else {
        return BTreeMap::new();
    };
    parse_annotations(&raw)
}

fn load_deep_contexts(dir: &Path) -> Vec<DeepContext> {
    let mut docs = Vec::new();

    // Load top-level .md files (except map.md)
    load_md_files(dir, &mut docs);

    // Load from subdirectories (flows/, etc.)
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                load_md_files(&path, &mut docs);
            }
        }
    }

    docs.sort_by(|a, b| a.name.cmp(&b.name));
    docs
}

fn load_md_files(dir: &Path, docs: &mut Vec<DeepContext>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        // Skip map.md — that's Layer 0, not deep context
        if path.file_name().and_then(|n| n.to_str()) == Some("map.md") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let (frontmatter, body) = split_frontmatter(&raw);
        let last_verified = extract_field(&frontmatter, "last_verified");

        // Derive name from path relative to context dir
        let context_dir = crate::config::dir_path().join("context");
        let name = path
            .strip_prefix(&context_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .trim_end_matches(".md")
            .to_string();

        docs.push(DeepContext {
            name,
            path,
            content: body.trim().to_string(),
            last_verified,
        });
    }
}

// ---------------------------------------------------------------------------
// Minimal parsers (no serde_yaml dependency)
// ---------------------------------------------------------------------------

/// Split markdown frontmatter (between --- delimiters) from body.
fn split_frontmatter(raw: &str) -> (String, String) {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return (String::new(), raw.to_string());
    }
    // Find the closing ---
    let after_first = &trimmed[3..];
    if let Some(end) = after_first.find("\n---") {
        let fm = after_first[..end].trim().to_string();
        let body = after_first[end + 4..].to_string();
        (fm, body)
    } else {
        (String::new(), raw.to_string())
    }
}

/// Extract a simple `key: value` field from frontmatter text.
fn extract_field(frontmatter: &str, key: &str) -> Option<String> {
    let prefix = format!("{}:", key);
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            let val = trimmed[prefix.len()..].trim().to_string();
            if val.is_empty() {
                return None;
            }
            return Some(val);
        }
    }
    None
}

/// Parse the annotations.yaml file into a map of file path → FileAnnotation.
/// Simple line-based YAML parser for our specific schema.
fn parse_annotations(raw: &str) -> BTreeMap<String, FileAnnotation> {
    let mut result = BTreeMap::new();
    let mut current_path: Option<String> = None;
    let mut current_ann = FileAnnotation::default();
    let mut current_section: Option<&str> = None;

    for line in raw.lines() {
        // Skip comments and blank lines
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }

        // Top-level key (file path) — no leading whitespace, ends with ':'
        if !line.starts_with(' ') && !line.starts_with('\t') && line.trim().ends_with(':') {
            // Save previous entry
            if let Some(path) = current_path.take() {
                result.insert(path, current_ann);
                current_ann = FileAnnotation::default();
            }
            let path = line.trim().trim_end_matches(':').to_string();
            current_path = Some(path);
            current_section = None;
            continue;
        }

        if current_path.is_none() {
            continue;
        }

        let trimmed = line.trim();

        // Section header: "  purpose:", "  key_types:", "  invariants:", etc.
        if trimmed.ends_with(':') && !trimmed.contains("- ") {
            let section_name = trimmed.trim_end_matches(':');
            match section_name {
                "purpose" | "key_types" | "invariants" | "hazards" | "examples" | "depends_on"
                | "depended_by" => {
                    current_section = Some(section_name);
                }
                _ => {
                    current_section = None;
                }
            }
            continue;
        }

        // "  purpose: "value"" — inline value
        if trimmed.starts_with("purpose:") {
            current_ann.purpose = unquote(trimmed.trim_start_matches("purpose:").trim());
            current_section = None;
            continue;
        }

        // List items under a section
        if let Some(section) = current_section {
            if let Some(item) = trimmed.strip_prefix("- ") {
                match section {
                    "invariants" => current_ann.invariants.push(unquote(item)),
                    "hazards" => current_ann.hazards.push(unquote(item)),
                    "examples" => current_ann.examples.push(unquote(item)),
                    _ => {}
                }
                continue;
            }
            // key_types: "  TypeName: description"
            if section == "key_types" {
                if let Some((name, desc)) = trimmed.split_once(':') {
                    current_ann
                        .key_types
                        .push((name.trim().to_string(), unquote(desc.trim())));
                }
            }
        }
    }

    // Save last entry
    if let Some(path) = current_path {
        result.insert(path, current_ann);
    }

    result
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_frontmatter_with_fences() {
        let raw = "---\nlast_verified: 2026-04-06\n---\n\n# Title\nBody here.";
        let (fm, body) = split_frontmatter(raw);
        assert_eq!(fm, "last_verified: 2026-04-06");
        assert!(body.contains("# Title"));
        assert!(body.contains("Body here."));
    }

    #[test]
    fn split_frontmatter_none() {
        let raw = "# No frontmatter\nJust body.";
        let (fm, body) = split_frontmatter(raw);
        assert!(fm.is_empty());
        assert!(body.contains("# No frontmatter"));
    }

    #[test]
    fn extract_field_found() {
        let fm = "last_verified: 2026-04-06\nauthor: test";
        assert_eq!(extract_field(fm, "last_verified"), Some("2026-04-06".into()));
        assert_eq!(extract_field(fm, "author"), Some("test".into()));
    }

    #[test]
    fn extract_field_missing() {
        assert_eq!(extract_field("foo: bar", "baz"), None);
    }

    #[test]
    fn parse_annotations_basic() {
        let yaml = r#"
crates/core/src/agent.rs:
  purpose: "Core Agent entity"
  key_types:
    Agent: "A running agent instance"
    AgentId: "UUID wrapper"
  invariants:
    - "AgentId is globally unique"
    - "name format is '{provider} #{hex8}'"
  hazards:
    - "Don't hold &Agent across mutations"

crates/app/src/app.rs:
  purpose: "Central application state"
  invariants:
    - "ptys and terms share the same keyset"
"#;
        let anns = parse_annotations(yaml);
        assert_eq!(anns.len(), 2);

        let agent = &anns["crates/core/src/agent.rs"];
        assert_eq!(agent.purpose, "Core Agent entity");
        assert_eq!(agent.key_types.len(), 2);
        assert_eq!(agent.key_types[0].0, "Agent");
        assert_eq!(agent.invariants.len(), 2);
        assert_eq!(agent.hazards.len(), 1);

        let app = &anns["crates/app/src/app.rs"];
        assert_eq!(app.purpose, "Central application state");
        assert_eq!(app.invariants.len(), 1);
    }

    #[test]
    fn parse_annotations_empty() {
        let anns = parse_annotations("");
        assert!(anns.is_empty());
    }

    #[test]
    fn parse_annotations_comments_skipped() {
        let yaml = "# This is a comment\n\n# Another comment\n";
        let anns = parse_annotations(yaml);
        assert!(anns.is_empty());
    }

    #[test]
    fn format_annotation_output() {
        let ann = FileAnnotation {
            purpose: "Test file".into(),
            key_types: vec![("Foo".into(), "A thing".into())],
            invariants: vec!["Must be true".into()],
            hazards: vec![],
            examples: vec![],
        };
        let text = format_annotation("src/test.rs", &ann);
        assert!(text.contains("# Context: src/test.rs"));
        assert!(text.contains("Purpose: Test file"));
        assert!(text.contains("Foo: A thing"));
        assert!(text.contains("Must be true"));
        assert!(!text.contains("Hazards"));
    }

    #[test]
    fn unquote_removes_quotes() {
        assert_eq!(unquote("\"hello\""), "hello");
        assert_eq!(unquote("no quotes"), "no quotes");
    }
}
