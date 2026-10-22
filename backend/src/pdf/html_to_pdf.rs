use crate::pdf::generator::build_simple_pdf;
use regex::Regex;
use serde_json::{json, Value};
use uuid::Uuid;

/// Renders a dynamic HTML document (`head` + `body`) into a real PDF document and extracts
/// any embedded `<signature-field>`, `<text-field>`, `<date-field>`, or `{{...}}` tags.
pub fn render_html_to_pdf_and_fields(
    title: &str,
    html_body: &str,
    attachment_uuid: &str,
    default_submitter_uuid: &str,
) -> (Vec<u8>, Vec<Value>) {
    // Strip HTML tags to clean readable lines for the PDF rendering
    let tag_strip_re = Regex::new(r"<[^>]+>").unwrap();
    let plain_text = tag_strip_re.replace_all(html_body, "\n");
    let lines: Vec<String> = plain_text
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let pdf_lines = if lines.is_empty() {
        vec![format!("Dynamic HTML Document: {}", title)]
    } else {
        lines
    };

    let pdf_bytes = build_simple_pdf(title, &pdf_lines);

    // Extract fields from custom HTML elements or {{...}} tags
    let (mut fields, _) = crate::pdf::processor::extract_fields_from_pdf(
        html_body.as_bytes(),
        attachment_uuid,
    );

    // Also inspect `<signature-field name="...">`, `<text-field name="...">`, `<date-field name="...">`
    let html_field_re =
        Regex::new(r#"<(signature|initials|text|date|checkbox|number)-field([^>]*)>"#).unwrap();
    let name_attr_re = Regex::new(r#"name=["']([^"']+)["']"#).unwrap();

    for (idx, cap) in html_field_re.captures_iter(html_body).enumerate() {
        let ftype = cap[1].to_string();
        let attrs = &cap[2];
        let name = name_attr_re
            .captures(attrs)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| format!("{} {}", ftype, idx + 1));

        fields.push(json!({
            "uuid": Uuid::new_v4().to_string(),
            "submitter_uuid": default_submitter_uuid,
            "name": name,
            "type": ftype,
            "required": true,
            "areas": [{
                "attachment_uuid": attachment_uuid,
                "page": 0,
                "x": 0.12,
                "y": (0.20 + (fields.len() as f64 * 0.08)).min(0.85),
                "w": 0.35,
                "h": 0.05
            }]
        }));
    }

    if fields.is_empty() {
        fields.push(json!({
            "uuid": Uuid::new_v4().to_string(),
            "submitter_uuid": default_submitter_uuid,
            "name": "Signature",
            "type": "signature",
            "required": true,
            "areas": [{
                "attachment_uuid": attachment_uuid,
                "page": 0,
                "x": 0.12,
                "y": 0.72,
                "w": 0.38,
                "h": 0.07
            }]
        }));
    }

    (pdf_bytes, fields)
}
