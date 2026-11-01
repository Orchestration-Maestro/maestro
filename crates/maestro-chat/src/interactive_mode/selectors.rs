//! Selector panels shown in place of the editor.
use super::InteractiveMode;
use crate::ThinkingSelectorComponent;
use maestro_tui::Component;
use std::{error::Error, rc::Rc};

/// A selector panel and the component that takes keyboard focus while it is shown.
pub(super) type Selector = (Rc<dyn Component>, Rc<dyn Component>);

impl InteractiveMode {
    /// Shows the current model's thinking levels; selection saves, cancellation only closes.
    pub(super) fn show_thinking_selector(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.show_selector(|done| Ok(self.thinking_selector(done)))
    }

    /// Builds the thinking picker with callbacks that do not retain the mode.
    fn thinking_selector(self: &Rc<Self>, done: Rc<dyn Fn()>) -> Selector {
        let session = self.runtime_host.session();
        let selected = Rc::clone(&done);
        let mode = Rc::downgrade(self);
        let cancelled = mode.clone();
        let selector = Rc::new(ThinkingSelectorComponent::new(
            session.thinking_level(),
            session.get_available_thinking_levels(),
            Rc::new(move |level| {
                let Some(mode) = mode.upgrade() else { return };
                let result = mode.select_thinking_level(level);
                selected();
                mode.ui.request_render(false);
                mode.report(result);
            }),
            Rc::new(move || {
                done();
                let Some(mode) = cancelled.upgrade() else {
                    return;
                };
                mode.ui.request_render(false);
            }),
            self.themes.theme(),
        ));
        let focus = selector.get_select_list().clone();
        (selector, focus)
    }

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
                mode.restore_editor();
            }
        });
        let (component, focus) = create(done)?;
        self.show_panel(component, focus);
        Ok(())
    }

    /// Shows `component` in place of whatever the editor container shows, focuses `focus`
    /// and requests a frame.
    pub(super) fn show_panel(&self, component: Rc<dyn Component>, focus: Rc<dyn Component>) {
        self.editor_container.clear();
        self.editor_container.add_child(component);
        self.ui.set_focus(Some(focus));
        self.ui.request_render(false);
    }

    /// Puts the editor back in place of whatever the editor container shows and focuses it,
    /// without requesting a frame.
    pub(super) fn restore_editor(&self) {
        let editor = self.current_editor();
        self.editor_container.clear();
        self.editor_container.add_child(editor.clone());
        self.ui.set_focus(Some(editor as Rc<dyn Component>));
    }
}
