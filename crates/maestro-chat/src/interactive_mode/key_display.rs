//! Displayed key names.
use super::InteractiveMode;

impl InteractiveMode {
    /// The keys currently bound to the application `action`, each part capitalized and
    /// spelled as the platform names it.
    pub(super) fn get_app_key_display(action: &str) -> String {
        maestro_app::key_display_text(action)
    }
}
