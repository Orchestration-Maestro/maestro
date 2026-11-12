//! Helpers that the pane read tests share between the integration program and the unit tests.

use maestro_protocol::schema::PaneInfo;
use serde_json::{Value, json};

/// A read request for `pane`.
#[must_use]
pub fn read(pane: &PaneInfo, source: &str, lines: Option<u32>, format: &str) -> Value {
    let mut wire = json!({
        "type": "pane.read", "pane_id": pane.pane_id, "source": source, "format": format
    });
    if let Some(lines) = lines {
        wire["lines"] = json!(lines);
    }
    wire
}
