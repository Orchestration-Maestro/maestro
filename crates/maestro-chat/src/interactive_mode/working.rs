//! The working row shown while a response runs.

use super::InteractiveMode;
use maestro_theme::{ThemeColor, paint};
use maestro_tui::Loader;
use std::rc::Rc;

impl InteractiveMode {
    /// Builds the working row: the toolkit's own indicator in the accent colour beside the
    /// muted default message, both painted by the theme published when they are drawn.
    pub(super) fn create_working_loader(&self) -> Loader {
        let color = |role: ThemeColor| -> Rc<dyn Fn(&str) -> String> {
            let theme = self.themes.theme();
            Rc::new(move |text| paint(theme.get().ok().as_deref(), &role, text))
        };
        Loader::new(
            self.ui.clone(),
            color(ThemeColor::Accent),
            color(ThemeColor::Muted),
            Some("Working...".to_owned()),
            None,
        )
    }

    /// Stops and releases the working row, then empties the status rows.
    pub(super) fn stop_working_loader(&self) {
        if let Some(loader) = self.loading_animation.take() {
            loader.stop();
        }
        self.status_container.clear();
    }
}
