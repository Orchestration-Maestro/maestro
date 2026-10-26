//! A custom editor for witnesses: the toolkit editor behind an interceptor, the interrupt and
//! the application actions its keybindings bind.
use maestro_extensions::types::{ActionHandlers, CustomEditorComponent, ExtensionShortcutCallback};
use maestro_extensions::{Component, InputHandler};
use maestro_tui::editor_component::TextCallback;
use maestro_tui::{Cancellation, Editor, EditorComponent, KeybindingsManager};
use std::{cell::RefCell, rc::Rc};

/// The toolkit editor with an interceptor, an interrupt handler and application actions.
pub struct CustomEditor {
    /// The toolkit editor.
    pub editor: Editor,
    /// The keybindings the actions are matched with.
    pub keybindings: KeybindingsManager,
    /// The application actions.
    pub handlers: ActionHandlers,
    /// The interrupt handler.
    pub escape: RefCell<Option<Rc<dyn Fn()>>>,
    /// The empty-editor exit handler.
    pub ctrl_d: RefCell<Option<Rc<dyn Fn()>>>,
    /// The image-paste handler.
    pub paste_image: RefCell<Option<Rc<dyn Fn()>>>,
    /// The interceptor.
    pub extension: RefCell<Option<ExtensionShortcutCallback>>,
}

impl Component for CustomEditor {
    fn render(&self, width: usize) -> Vec<String> {
        self.editor.render(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn maestro_extensions::widgets::Focusable> {
        self.editor.focusable()
    }
}

impl InputHandler for CustomEditor {
    fn handle_input(&self, data: &str) {
        let shortcut = self.extension.borrow().clone();
        if shortcut.is_some_and(|shortcut| shortcut(data)) {
            return;
        }
        let escape = self.escape.borrow().clone();
        if let Some(escape) = escape.filter(|_| self.keybindings.matches(data, "app.interrupt")) {
            return escape();
        }
        let handlers = self.handlers.clone();
        let action = handlers
            .borrow()
            .iter()
            .find(|(action, _)| self.keybindings.matches(data, action))
            .map(|(_, handler)| Rc::clone(handler));
        match action {
            Some(action) => action(),
            None => self.editor.handle_input(data),
        }
    }
}

impl EditorComponent for CustomEditor {
    type Signal = Cancellation;
    fn get_text(&self) -> String {
        self.editor.get_text()
    }
    fn set_text(&self, text: &str) {
        self.editor.set_text(text);
    }
    fn on_submit(&self) -> Option<TextCallback> {
        EditorComponent::on_submit(&self.editor)
    }
    fn set_on_submit(&self, callback: Option<TextCallback>) {
        self.editor.set_on_submit(callback);
    }
    fn on_change(&self) -> Option<TextCallback> {
        EditorComponent::on_change(&self.editor)
    }
    fn set_on_change(&self, callback: Option<TextCallback>) {
        self.editor.set_on_change(callback);
    }
}

impl CustomEditorComponent for CustomEditor {
    fn action_handlers(&self) -> ActionHandlers {
        self.handlers.clone()
    }
    fn on_escape(&self) -> Option<Rc<dyn Fn()>> {
        self.escape.borrow().clone()
    }
    fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>) {
        self.escape.replace(callback);
    }
    fn on_ctrl_d(&self) -> Option<Rc<dyn Fn()>> {
        self.ctrl_d.borrow().clone()
    }
    fn set_on_ctrl_d(&self, callback: Option<Rc<dyn Fn()>>) {
        self.ctrl_d.replace(callback);
    }
    fn on_paste_image(&self) -> Option<Rc<dyn Fn()>> {
        self.paste_image.borrow().clone()
    }
    fn set_on_paste_image(&self, callback: Option<Rc<dyn Fn()>>) {
        self.paste_image.replace(callback);
    }
    fn on_extension_shortcut(&self) -> Option<ExtensionShortcutCallback> {
        self.extension.borrow().clone()
    }
    fn set_on_extension_shortcut(&self, callback: Option<ExtensionShortcutCallback>) {
        self.extension.replace(callback);
    }
}

impl maestro_extensions::widgets::ExtensionEditor for CustomEditor {
    fn editor(&self) -> &Editor {
        &self.editor
    }
}
