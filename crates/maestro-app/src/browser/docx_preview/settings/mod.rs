//! Word document settings preparation.
#[path = "settings.rs"]
mod properties;
mod settings_part;
pub use properties::{NoteProperties, WmlSettings};
pub use settings_part::SettingsPart;
