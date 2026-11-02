//! Rendered values of a prepared status card, projected for native tests.
use super::{Body, Caption, CardContent};
use crate::{components::console_block::ConsoleVariant, tools::artifacts::artifact_pill::Pill};
use serde_json::{Value, json};

/// The semantic color name the console paints.
pub(in crate::tools) fn variant(variant: ConsoleVariant) -> &'static str {
    match variant {
        ConsoleVariant::Default => "default",
        ConsoleVariant::Error => "error",
    }
}
impl Pill {
    /// Displayed text, activation identity and whether a panel opens on activation.
    fn view(&self) -> Value {
        json!({"text": self.text, "filename": self.filename, "panel": self.panel.is_some()})
    }
}
impl Caption {
    /// Literal text, or the untranslated key with its literal suffix or filename pill.
    pub(in crate::tools) fn view(&self) -> Value {
        match self {
            Self::Literal(text) => json!({"text": text}),
            Self::Localized(key, suffix) => json!({"key": key, "suffix": suffix}),
            Self::Artifact(key, pill) => json!({"key": key, "pill": pill.as_ref().map(Pill::view)}),
        }
    }
}
impl Body {
    /// The selected branch below the header; `null` for a header-only card.
    fn view(&self) -> Value {
        match self {
            Self::Empty => Value::Null,
            Self::Console(text, color) => json!({"console": text, "variant": variant(*color)}),
            Self::Error(text) => json!({"error": text}),
        }
    }
}
impl CardContent {
    /// Header status, glyph and caption with the selected body.
    pub(in crate::tools) fn view(&self) -> Value {
        json!({
            "state": format!("{:?}", self.state),
            "glyph": self.glyph,
            "caption": self.caption.view(),
            "body": self.body.view(),
        })
    }
}
