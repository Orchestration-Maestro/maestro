//! The current UI context and the headless fallback.
use super::ExtensionRunner;
use crate::{
    ExtensionExecution,
    types::{ExtensionUIContext, ExtensionUIContextPort},
};
use maestro_theme::LiveTheme;
use std::rc::Rc;

/// The defined headless results, with the runner's live theme.
struct NoUiContext(LiveTheme);
impl ExtensionUIContextPort for NoUiContext {
    fn theme(&self) -> LiveTheme {
        self.0.clone()
    }
}
impl ExtensionUIContext {
    /// The headless fallback, which never counts as an installed host.
    pub(crate) fn no_ui(theme: LiveTheme) -> Self {
        Self {
            port: Rc::new(NoUiContext(theme)),
            fallback: true,
        }
    }
}
impl<C, CC, E: ExtensionExecution> ExtensionRunner<C, CC, E> {
    /// Installs a UI context, or restores this runner's headless fallback with `None`.
    pub fn set_ui_context(&self, context: Option<ExtensionUIContext>) {
        let context = context.unwrap_or_else(|| self.no_ui.clone());
        drop(self.ui.replace(context));
    }
    /// Returns the current UI context; it keeps its host after later changes.
    #[must_use]
    pub fn get_ui_context(&self) -> ExtensionUIContext {
        self.ui.borrow().clone()
    }
    /// Whether an installed context, rather than a headless fallback, is current.
    #[must_use]
    pub fn has_ui(&self) -> bool {
        !self.ui.borrow().fallback
    }
}
