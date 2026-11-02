//! The current UI context and the headless fallback.
use super::ExtensionRunner;
use crate::{
    ExtensionExecution,
    types::{ExtensionUIContext, ExtensionUIContextBinding, ExtensionUIContextPort},
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
            error_reporter: None,
        }
    }
}
impl<E: ExtensionExecution> ExtensionRunner<E> {
    /// Installs a UI context, or restores this runner's headless fallback with `None`.
    pub fn set_ui_context(&self, context: Option<ExtensionUIContext>) {
        self.set_ui_context_binding(context.map(Into::into));
    }
    /// Installs the supplied ownership of a host without forwarding its operations.
    pub fn set_ui_context_binding(&self, context: Option<ExtensionUIContextBinding>) {
        let context = context.unwrap_or_else(|| self.binding.no_ui.clone().into());
        drop(
            self.binding
                .ui_error_reporter
                .replace(Some(self.ui_error_reporter())),
        );
        drop(self.binding.ui.replace(context));
    }
    /// Returns the current UI context; it keeps its host after later changes.
    #[must_use]
    pub fn get_ui_context(&self) -> ExtensionUIContext {
        self.binding.get_ui_context()
    }
    /// Whether an installed context, rather than a headless fallback, is current.
    #[must_use]
    pub fn has_ui(&self) -> bool {
        self.binding.has_ui()
    }
}
