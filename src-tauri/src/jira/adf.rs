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
