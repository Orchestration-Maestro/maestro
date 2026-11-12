//! Helpers that the pane input tests share between the integration program and the unit tests.

use super::api::answer;
use crate::state::App;
use maestro_protocol::schema::ResponseResult;
use serde_json::json;

/// Sends `keys` to `pane` and checks that the daemon accepted them.
///
/// # Panics
/// Panics when the daemon does not accept the keys.
pub async fn send_keys(app: &mut App, pane: &str, keys: &[&str]) {
    let wire = json!({"type": "pane.send_keys", "pane_id": pane, "keys": keys});
    assert_eq!(answer(app, wire).await, Ok(ResponseResult::Ok {}));
}
