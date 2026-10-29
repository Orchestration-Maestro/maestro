//! The `manifest` setting: which release a settings document names.

use serde_json::Value;

/// The resource filters a package entry may carry.
const FILTERS: [&str; 4] = ["extensions", "skills", "prompts", "themes"];

/// Reads the manifest source a raw settings document names.
///
/// An absent key, an empty string or an object with an empty `source` names nothing.
///
/// # Errors
/// Returns the problem with the value when it is neither a source string nor an object whose
/// `source` is a string and whose resource filters are lists of strings.
pub fn parse(document: &Value) -> Result<Option<String>, String> {
    let Some(value) = document.get("manifest") else {
        return Ok(None);
    };
    let source = match value {
        Value::String(source) => source,
        Value::Object(entry) => {
            if let Some(filter) = FILTERS.into_iter().find(|filter| {
                entry
                    .get(*filter)
                    .is_some_and(|list| !is_list_of_strings(list))
            }) {
                return Err(format!("`{filter}` must be a list of strings"));
            }
            match entry.get("source") {
                Some(Value::String(source)) => source,
                _ => return Err("the entry needs a `source` that is a string".into()),
            }
        }
        _ => {
            return Err(
                "the value must be a Git source string or an object with a `source`".into(),
            );
        }
    };
    let source = source.trim();
    Ok((!source.is_empty()).then(|| source.to_owned()))
}

/// Whether a filter value is a list whose items are all strings.
fn is_list_of_strings(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().all(Value::is_string))
}

/// Splits a verified pinned source into its repository text and its ref.
///
/// The ref follows the last `@`. The repository is the text as written, so two spellings of
/// one repository differ.
#[must_use]
pub fn split_pin(source: &str) -> (&str, &str) {
    source.rsplit_once('@').unwrap_or((source, ""))
}

/// Whether a project selection is set aside because the user's manifest locks `manifest`
/// and the project names another repository.
#[must_use]
pub fn project_ignored(user: &str, project: &str, user_locks_manifest: bool) -> bool {
    user_locks_manifest && split_pin(user).0 != split_pin(project).0
}
