//! Helpers that the pane zoom tests share between the integration program and the unit tests.

use super::api::events_after;
use crate::state::App;
use maestro_protocol::events::EventData;
use maestro_protocol::schema::{ErrorBody, PaneLayoutSnapshot, PaneZoomResult, ResponseResult};

/// The zoom record a zoom result holds.
///
/// # Errors
/// Returns the answer that was not a zoom result.
pub fn zoom_of(result: Result<ResponseResult, ErrorBody>) -> Result<PaneZoomResult, String> {
    match result {
        Ok(ResponseResult::PaneZoom { zoom }) => Ok(zoom),
        other => Err(format!("expected a zoom result: {other:?}")),
    }
}

/// The layout carried by the last published event.
///
/// # Errors
/// Returns the last event when it is not a layout update.
pub fn last_layout(app: &App) -> Result<PaneLayoutSnapshot, String> {
    match events_after(app, 0).pop() {
        Some(EventData::LayoutUpdated { layout }) => Ok(layout),
        other => Err(format!("expected a layout event last: {other:?}")),
    }
}
