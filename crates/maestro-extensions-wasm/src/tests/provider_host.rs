//! The completion providers both test hosts keep: the host's own provider and the authored
//! providers stacked on it.
//!
//! The host's provider suggests `base` for the text before the cursor unless completion was
//! explicitly not forced, applies a candidate in place of its prefix, and asks for file
//! completion on a line that starts with `@`.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::{Value, json};

use super::observed::Observed;

/// A candidate: its value, label and description.
pub type Candidate = (String, String, Option<String>);

/// Candidates and the text they replace.
pub type Found = (Vec<Candidate>, String);

/// One host request on the authored provider on top of the stack.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderOp {
    /// Suggestions with the signal the driver lent under `signal`.
    Suggestions {
        /// The edited lines.
        lines: Vec<String>,
        /// The cursor.
        cursor: (u32, u32),
        /// Whether completion was forced.
        force: Option<bool>,
        /// The driver's handle of the lent signal.
        signal: usize,
    },
    /// Applies a chosen candidate in place of its prefix.
    Apply {
        /// The edited lines.
        lines: Vec<String>,
        /// The cursor.
        cursor: (u32, u32),
        /// The chosen candidate.
        item: Candidate,
        /// The text it replaces.
        prefix: String,
    },
    /// Whether explicit completion should offer files.
    Trigger {
        /// The edited lines.
        lines: Vec<String>,
        /// The cursor.
        cursor: (u32, u32),
    },
}

/// Reads one request.
///
/// # Errors
/// Returns the message of a malformed request.
pub fn op(request: Value) -> Result<ProviderOp, String> {
    serde_json::from_value(request).map_err(|error| error.to_string())
}

/// A described completion.
pub fn applied(lines: &[String], line: u32, col: u32) -> Value {
    json!({ "applied": [lines, line, col] })
}

/// A described file-completion decision.
pub fn decided(decision: Option<bool>) -> Value {
    json!({ "trigger": decision })
}

/// A lent provider: the host's own, or an authored one the host keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lent {
    /// The host's own provider.
    Base,
    /// The authored provider under this identity.
    Authored(u32),
}

/// The authored providers the host stacked, in the order they were added.
#[derive(Default)]
pub struct Stack(pub Vec<u32>);

/// Records a provider factory request against the acquisition `key` and returns the
/// provider the factory receives.
pub fn add(observed: &mut Observed, key: usize) -> Result<Lent, String> {
    super::ui_host::call(observed, key, "addAutocompleteProvider", &json!([]))?;
    Ok(observed
        .ui
        .providers
        .0
        .last()
        .map_or(Lent::Base, |id| Lent::Authored(*id)))
}

/// Keeps the provider a factory returned on top of the stack.
pub fn push(observed: &mut Observed, provider: u32) {
    observed.log("provider stacked");
    observed.ui.providers.0.push(provider);
}

/// The authored provider on top of the stack.
pub fn top(observed: &Observed) -> Result<u32, String> {
    observed
        .ui
        .providers
        .0
        .last()
        .copied()
        .ok_or_else(|| "no authored provider".to_owned())
}

/// The text before the cursor of its line.
fn before(lines: &[String], (line, col): (u32, u32)) -> String {
    let line = usize::try_from(line).ok().and_then(|line| lines.get(line));
    let col = usize::try_from(col).unwrap_or(usize::MAX);
    line.map(|line| line.get(..col.min(line.len())).unwrap_or(line).to_owned())
        .unwrap_or_default()
}

/// The host's suggestions: `base` replacing the text before the cursor.
pub fn suggestions(
    observed: &mut Observed,
    lines: &[String],
    cursor: (u32, u32),
    (force, aborted): (Option<bool>, bool),
) -> Option<Found> {
    observed.log(format!(
        "provider suggestions {}",
        json!([lines, cursor, force, aborted])
    ));
    (force != Some(false)).then(|| {
        let item = (
            "base".to_owned(),
            "Base".to_owned(),
            Some("host".to_owned()),
        );
        (vec![item], before(lines, cursor))
    })
}

/// The host's completion: the line with `prefix` before the cursor replaced by `value`.
pub fn apply(
    observed: &mut Observed,
    lines: &[String],
    cursor: (u32, u32),
    (value, prefix): (&str, &str),
) -> (Vec<String>, u32, u32) {
    observed.log(format!(
        "provider apply {}",
        json!([lines, cursor, value, prefix])
    ));
    let mut lines = lines.to_vec();
    let index = usize::try_from(cursor.0).unwrap_or(usize::MAX);
    let col = usize::try_from(cursor.1).unwrap_or(usize::MAX);
    let Some(line) = lines.get_mut(index) else {
        return (lines, cursor.0, cursor.1);
    };
    let end = col.min(line.len());
    let start = end.saturating_sub(prefix.len());
    line.replace_range(start..end, value);
    let col = u32::try_from(start + value.len()).unwrap_or(u32::MAX);
    (lines, cursor.0, col)
}

/// The host's file-completion decision: asked on a line that starts with `@`.
pub fn trigger(observed: &mut Observed, lines: &[String], cursor: (u32, u32)) -> Option<bool> {
    observed.log(format!("provider trigger {}", json!([lines, cursor])));
    let line = usize::try_from(cursor.0)
        .ok()
        .and_then(|line| lines.get(line));
    line.is_some_and(|line| line.starts_with('@'))
        .then_some(true)
}

/// A described suggestion answer.
pub fn described(found: Option<Found>) -> Value {
    json!({ "suggestions": serde_json::to_value(found).unwrap_or_default() })
}
