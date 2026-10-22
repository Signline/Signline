use crate::error::AppResult;
use crate::models::SearchEntryRecord;
use crate::pb::pb_escape;
use crate::state::AppState;
use serde_json::json;

/// Builds 3-character ngrams for fast case-insensitive substring matching.
pub fn build_ngrams(text: &str) -> String {
    let normalized: String = text
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let chars: Vec<char> = normalized.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect();
    if chars.len() <= 3 {
        return chars.into_iter().collect();
    }
    let mut out = Vec::new();
    for window in chars.windows(3) {
        out.push(window.iter().collect::<String>());
    }
    out.join(" ")
}

/// Upserts a `search_entries` document for unified full-text search across Templates, Submissions, and Submitters.
pub async fn index_search_entry(
    state: &AppState,
    account_id: &str,
    record_type: &str,
    record_id: &str,
    content: &str,
) -> AppResult<SearchEntryRecord> {
    let ngram = build_ngrams(content);
    let filter = format!(
        "record_type = '{}' && record_id = '{}'",
        pb_escape(record_type),
        pb_escape(record_id)
    );
    if let Some(existing) = state
        .pb
        .find_first::<SearchEntryRecord>("search_entries", &filter, None)
        .await?
    {
        state
            .pb
            .update(
                "search_entries",
                &existing.id,
                &json!({
                    "content": content,
                    "ngram": ngram,
                }),
            )
            .await
    } else {
        state
            .pb
            .create(
                "search_entries",
                &json!({
                    "account_id": account_id,
                    "record_type": record_type,
                    "record_id": record_id,
                    "content": content,
                    "ngram": ngram,
                }),
            )
            .await
    }
}
