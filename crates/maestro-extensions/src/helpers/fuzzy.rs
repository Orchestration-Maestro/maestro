//! Fuzzy requests delegate scoring and ranking to the terminal owner.
use super::{decode, encode};
/// Runs one fuzzy request without retaining its operands.
pub(super) fn call(operation: &str, data: &str) -> Result<String, String> {
    match operation {
        "fuzzy.match" => {
            let (query, text): (String, String) = decode(data)?;
            let found = maestro_tui::fuzzy_match(&query, &text);
            encode(&serde_json::json!({"matches":found.matches,"score":found.score}))
        }
        "fuzzy.project" => {
            let (query,): (String,) = decode(data)?;
            let projected = std::cell::Cell::new(false);
            let _ = maestro_tui::fuzzy_filter(&[()], &query, |()| {
                projected.set(true);
                ""
            });
            encode(&projected.get())
        }
        "fuzzy.filter" => {
            let (texts, query): (Vec<String>, String) = decode(data)?;
            let items: Vec<_> = texts.into_iter().enumerate().collect();
            let selected: Vec<_> =
                maestro_tui::fuzzy_filter(&items, &query, |item| item.1.as_str())
                    .into_iter()
                    .map(|item| item.0)
                    .collect();
            encode(&selected)
        }
        _ => Err(format!("Unknown native helper: {operation}")),
    }
}
