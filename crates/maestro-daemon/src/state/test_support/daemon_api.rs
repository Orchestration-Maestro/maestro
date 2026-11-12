//! Helpers that the daemon API tests share between the integration program and the unit tests.

use super::api::request;
use crate::state::App;
use maestro_protocol::schema::{ErrorBody, ResponseResult};
use serde_json::{Value, json};
use std::collections::HashMap;

/// Reports `tokens` from `source` for workspace `w1`, with `extra` members.
///
/// # Errors
/// Returns the daemon's refusal.
pub fn report(
    app: &mut App,
    source: &str,
    tokens: &Value,
    extra: &Value,
) -> Result<ResponseResult, ErrorBody> {
    let mut command = json!({
        "type": "workspace.report_metadata",
        "workspace_id": "w1",
        "source": source,
        "tokens": tokens,
    });
    if let (Some(command), Some(extra)) = (command.as_object_mut(), extra.as_object()) {
        command.extend(extra.clone());
    }
    request(app, command)
}

/// The tokens workspace `w1` currently holds.
#[must_use]
pub fn tokens(app: &App) -> HashMap<String, String> {
    app.state.topology.workspaces[0].metadata_tokens.values()
}

/// The token values of `pairs`.
#[must_use]
pub fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}
