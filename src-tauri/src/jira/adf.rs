//! Atlassian Document Format (ADF) helpers.

use serde_json::{json, Value};

/// Build a minimal ADF doc from plain text.
///
/// Returns `None` when the input is empty or whitespace-only — callers can decide
/// whether to omit the field entirely from the request body.
pub fn make_adf_comment(text: &str) -> Option<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(json!({
        "type": "doc",
        "version": 1,
        "content": [
            {
                "type": "paragraph",
                "content": [
                    { "type": "text", "text": trimmed }
                ]
            }
        ]
    }))
}

/// Walk an ADF document tree and extract all plain-text content.
///
/// Concatenates `type == "text"` nodes (depth-first), inserting a newline
/// between sibling block-level nodes (paragraphs, list items, headings, …) so
/// the result still reads naturally. Inline-level nodes are joined with a
/// single space.
///
/// Handles a `null` / non-object input by returning the empty string — that's
/// what Jira does when a worklog has no comment.
pub fn extract_adf_text(value: &Value) -> String {
    let mut out = String::new();
    walk(value, &mut out);
    // Trim trailing whitespace introduced by the block separators.
    while out.ends_with('\n') || out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Names of node types that represent a "block" boundary in ADF. After
/// visiting one of these we emit a newline so paragraphs don't run together.
const BLOCK_TYPES: &[&str] = &[
    "paragraph",
    "heading",
    "blockquote",
    "bulletList",
    "orderedList",
    "listItem",
    "codeBlock",
    "rule",
    "panel",
    "table",
    "tableRow",
    "tableCell",
    "tableHeader",
    "mediaSingle",
    "mediaGroup",
];

fn walk(value: &Value, out: &mut String) {
    match value {
        Value::Object(obj) => {
            // Plain text leaf.
            if obj.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                    out.push_str(text);
                }
                return;
            }

            // Hardbreak — represent as a newline.
            if obj.get("type").and_then(|t| t.as_str()) == Some("hardBreak") {
                out.push('\n');
                return;
            }

            let node_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let is_block = BLOCK_TYPES.contains(&node_type);

            // Recurse into content (most ADF nodes use this key).
            if let Some(content) = obj.get("content") {
                walk(content, out);
            }

            if is_block && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        Value::Array(arr) => {
            for (i, child) in arr.iter().enumerate() {
                if i > 0 {
                    // Insert a thin separator between inline siblings if the
                    // previous run did not already end in whitespace.
                    let needs_space = !out.is_empty()
                        && !out.ends_with(' ')
                        && !out.ends_with('\n');
                    if needs_space {
                        out.push(' ');
                    }
                }
                walk(child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_from_simple_paragraph() {
        let doc = json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "Hello world" }
                    ]
                }
            ]
        });
        assert_eq!(extract_adf_text(&doc), "Hello world");
    }

    #[test]
    fn joins_multiple_paragraphs_with_newline() {
        let doc = json!({
            "type": "doc",
            "version": 1,
            "content": [
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "First" }]
                },
                {
                    "type": "paragraph",
                    "content": [{ "type": "text", "text": "Second" }]
                }
            ]
        });
        assert_eq!(extract_adf_text(&doc), "First\nSecond");
    }

    #[test]
    fn handles_nested_marks_and_inline_text() {
        let doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "Bold ", "marks": [{ "type": "strong" }] },
                        { "type": "text", "text": "and italic", "marks": [{ "type": "em" }] }
                    ]
                }
            ]
        });
        // Inline siblings get a space if there isn't already one.
        assert_eq!(extract_adf_text(&doc), "Bold and italic");
    }

    #[test]
    fn empty_value_returns_empty_string() {
        assert_eq!(extract_adf_text(&Value::Null), "");
        assert_eq!(extract_adf_text(&json!({})), "");
    }

    #[test]
    fn hard_break_emits_newline() {
        let doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        { "type": "text", "text": "line1" },
                        { "type": "hardBreak" },
                        { "type": "text", "text": "line2" }
                    ]
                }
            ]
        });
        assert_eq!(extract_adf_text(&doc), "line1\nline2");
    }

    #[test]
    fn bullet_list_separates_items() {
        let doc = json!({
            "type": "doc",
            "content": [
                {
                    "type": "bulletList",
                    "content": [
                        { "type": "listItem", "content": [
                            { "type": "paragraph", "content": [
                                { "type": "text", "text": "one" }
                            ]}
                        ]},
                        { "type": "listItem", "content": [
                            { "type": "paragraph", "content": [
                                { "type": "text", "text": "two" }
                            ]}
                        ]}
                    ]
                }
            ]
        });
        let got = extract_adf_text(&doc);
        assert!(got.contains("one"));
        assert!(got.contains("two"));
        assert!(got.contains('\n'));
    }
}
