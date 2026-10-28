//! Hidden WAD selection and one-time playback requests across fresh instances.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionContext, ExtensionResult, SessionEntry};
use serde::Deserialize;
use serde_json::json;

/// Custom state is excluded from chat messages and model context.
const STATE_TYPE: &str = "doom-overlay-state";

/// The latest hidden selection in session admission order.
#[derive(Deserialize)]
pub struct Selection {
    /// The entry identity used by a matching consumption marker.
    #[serde(skip)]
    pub id: String,
    /// Native WAD path chosen by the user or discovery.
    pub wad: String,
    /// Whether a fresh instance should open this choice once.
    pub pending: bool,
}

/// Reads only the newest matching custom entry.
/// # Errors
/// Returns session-reader or malformed saved-state failures.
pub fn saved(ctx: &ExtensionContext) -> ExtensionResult<Option<Selection>> {
    for entry in ctx.session_manager()?.get_entries()?.into_iter().rev() {
        if let SessionEntry::Custom(entry) = entry
            && entry.custom_type == STATE_TYPE
        {
            let Some(data) = entry.data.present() else {
                return Ok(None);
            };
            let mut selection: Selection =
                serde_json::from_str(data).map_err(|error| error.to_string())?;
            selection.id = entry.base.id;
            return Ok(Some(selection));
        }
    }
    Ok(None)
}

/// Records normal selection without requesting another view.
/// # Errors
/// Returns the original custom-entry write failure.
pub fn remember(api: &ExtensionAPI, wad: &str) -> ExtensionResult<()> {
    api.append_entry(STATE_TYPE, Some(json!({"wad": wad, "pending": false})))
}

/// Requests one view from the replacement instance.
/// # Errors
/// Returns the original custom-entry write failure.
pub fn request(api: &ExtensionAPI, wad: &str) -> ExtensionResult<()> {
    api.append_entry(STATE_TYPE, Some(json!({"wad": wad, "pending": true})))
}

/// Consumes before opening, so later startup or resume events never retry it.
/// # Errors
/// Returns the original custom-entry write failure.
pub fn consume(api: &ExtensionAPI, selection: &Selection) -> ExtensionResult<()> {
    api.append_entry(
        STATE_TYPE,
        Some(json!({"wad": selection.wad, "pending": false, "consumed": selection.id})),
    )
}
