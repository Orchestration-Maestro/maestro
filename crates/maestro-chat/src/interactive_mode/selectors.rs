//! Selector panels shown in place of the editor.
use super::InteractiveMode;
use maestro_tui::Component;
use std::{error::Error, rc::Rc};

/// A selector panel and the component that takes keyboard focus while it is shown.
pub(super) type Selector = (Rc<dyn Component>, Rc<dyn Component>);

impl InteractiveMode {
    /// Shows the panel `create` builds in place of the editor, focuses its target and
    /// requests a frame. `create` receives the completion that puts the editor back and
    /// focuses it, without requesting a frame or keeping the mode alive.
    ///
    /// # Errors
    /// Returns the failure of `create`, which leaves the editor and focus untouched.
    pub(super) fn show_selector(
        self: &Rc<Self>,
        create: impl FnOnce(Rc<dyn Fn()>) -> Result<Selector, Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let mode = Rc::downgrade(self);
        let done = Rc::new(move || {
            if let Some(mode) = mode.upgrade() {
                mode.editor_container.clear();
                mode.editor_container.add_child(mode.editor.clone());
                mode.ui.set_focus(Some(mode.editor.clone()));
            }
        });
        let (component, focus) = create(done)?;
        self.editor_container.clear();
        self.editor_container.add_child(component);
        self.ui.set_focus(Some(focus));
        self.ui.request_render(false);
        Ok(())
    }
}
