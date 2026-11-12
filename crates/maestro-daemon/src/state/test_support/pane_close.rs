//! Helpers that the pane close tests share between the integration program and the unit tests.

use super::api::request;
use crate::state::App;
use maestro_protocol::schema::{ErrorBody, ResponseResult};
use serde_json::json;

/// Closes the pane with the public handle `pane`.
///
/// # Errors
/// Returns the daemon's refusal.
pub fn close(app: &mut App, pane: &str) -> Result<ResponseResult, ErrorBody> {
    request(app, json!({"type": "pane.close", "pane_id": pane}))
}
