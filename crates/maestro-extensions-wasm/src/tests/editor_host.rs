//! The host requests both test hosts make on the authored editor they show, and how each
//! host describes the answer.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::{Value, json};

/// One host request on an authored editor; lent callbacks and styles are named.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum EditorOp {
    /// The text.
    GetText,
    /// Replaces the text.
    SetText {
        /// The text.
        text: String,
    },
    /// Replaces or removes the submission callback with the lent callback `callback`.
    SetOnSubmit {
        /// The lent callback's name.
        callback: Option<String>,
    },
    /// The change callback.
    OnChange,
    /// Replaces or removes the change callback with the lent callback `callback`.
    SetOnChange {
        /// The lent callback's name.
        callback: Option<String>,
    },
    /// Adds text to the history.
    AddToHistory {
        /// The text.
        text: String,
    },
    /// Inserts text at the cursor.
    InsertTextAtCursor {
        /// The text.
        text: String,
    },
    /// The text with markers expanded.
    GetExpandedText,
    /// The border style.
    BorderColor,
    /// Replaces the border style with the host style `role`.
    SetBorderColor {
        /// The host style's role.
        role: String,
    },
    /// Replaces the side padding.
    SetPaddingX {
        /// The padding.
        padding: f64,
    },
    /// Replaces the completion row count.
    SetAutocompleteMaxVisible {
        /// The row count.
        max: f64,
    },
    /// Replaces the completion provider with the host's own.
    SetAutocompleteProvider,
}

/// Reads one request.
///
/// # Errors
/// Returns the message of a malformed request.
pub fn op(request: Value) -> Result<EditorOp, String> {
    serde_json::from_value(request).map_err(|error| error.to_string())
}

/// The text a host passes to a change handler an editor answered with.
pub const PROBE: &str = "probe";

/// The text a host styles with a border style an editor answered with.
pub const BORDER: &str = "border";

/// An operation that ran.
pub fn done() -> Value {
    json!("done")
}

/// An operation the editor lacks.
pub fn absent() -> Value {
    json!("absent")
}

/// Answered text.
pub fn text(text: &str) -> Value {
    json!({ "text": text })
}

/// Whether a change handler was answered; a host calls an answered one with [`PROBE`].
pub fn handler(present: bool) -> Value {
    json!({ "handler": present })
}

/// An answered border style's rendering of [`BORDER`].
pub fn style(styled: &str) -> Value {
    json!({ "style": styled })
}
