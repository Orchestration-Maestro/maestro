//! Application action interception around the toolkit editor.
use maestro_tui::autocomplete::AutocompleteProvider;
use maestro_tui::editor_component::{BorderColor, TextCallback};
use maestro_tui::tui::InputHandler;
use maestro_tui::{
    Cancellation, Component, Editor, EditorComponent, EditorOptions, EditorTheme, FocusFlag,
    Focusable, KeybindingsManager, TUI,
};
use std::{cell::RefCell, ops::Deref, rc::Rc};

/// Shared insertion-ordered handlers, independent of the editor's replaceable map slot.
pub use maestro_app::ActionHandlers;
use maestro_app::{CustomEditorComponent, ExtensionShortcutCallback as ExtensionShortcut};
/// Toolkit editing with ordered application action interception.
pub struct CustomEditor {
    /// Inherited editor owner.
    editor: Editor,
    /// Supplied live binding manager.
    keybindings: KeybindingsManager,
    /// Replaceable shared ordered map.
    handlers: RefCell<ActionHandlers>,
    /// Dynamic interrupt handler.
    escape: RefCell<Option<Rc<dyn Fn()>>>,
    /// Dynamic empty-editor exit handler.
    ctrl_d: RefCell<Option<Rc<dyn Fn()>>>,
    /// Dynamic image-paste handler.
    paste_image: RefCell<Option<Rc<dyn Fn()>>>,
    /// First input interceptor.
    extension: RefCell<Option<ExtensionShortcut>>,
}
impl CustomEditor {
    /// Retains the supplied live manager and forwards editor construction to its owner.
    #[must_use]
    pub fn new(
        tui: &TUI,
        theme: EditorTheme,
        keybindings: KeybindingsManager,
        options: EditorOptions,
    ) -> Self {
        Self {
            editor: Editor::new(tui, theme, options),
            keybindings,
            handlers: RefCell::default(),
            escape: RefCell::default(),
            ctrl_d: RefCell::default(),
            paste_image: RefCell::default(),
            extension: RefCell::default(),
        }
    }
    /// Inserts or replaces an action without changing its existing position.
    pub fn on_action(&self, action: String, handler: Rc<dyn Fn()>) {
        let old = self.action_handlers().borrow_mut().insert(action, handler);
        drop(old);
    }
    /// Returns an alias to the currently installed map.
    pub fn action_handlers(&self) -> ActionHandlers {
        self.handlers.borrow().clone()
    }
    /// Replaces the map slot, leaving surviving aliases on their original map.
    pub fn set_action_handlers(&self, handlers: ActionHandlers) {
        let old = self.handlers.replace(handlers);
        drop(old);
    }
    /// Returns the installed interrupt callable.
    pub fn on_escape(&self) -> Option<Rc<dyn Fn()>> {
        self.escape.borrow().clone()
    }
    /// Replaces interrupt notification.
    pub fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.escape.replace(callback);
        drop(old);
    }
    /// Returns the installed empty-editor exit callable.
    pub fn on_ctrl_d(&self) -> Option<Rc<dyn Fn()>> {
        self.ctrl_d.borrow().clone()
    }
    /// Replaces empty-editor exit notification.
    pub fn set_on_ctrl_d(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.ctrl_d.replace(callback);
        drop(old);
    }
    /// Returns the installed image-paste callable.
    pub fn on_paste_image(&self) -> Option<Rc<dyn Fn()>> {
        self.paste_image.borrow().clone()
    }
    /// Replaces image-paste notification.
    pub fn set_on_paste_image(&self, callback: Option<Rc<dyn Fn()>>) {
        let old = self.paste_image.replace(callback);
        drop(old);
    }
    /// Returns the installed first interceptor.
    pub fn on_extension_shortcut(&self) -> Option<ExtensionShortcut> {
        self.extension.borrow().clone()
    }
    /// Replaces the first interceptor.
    pub fn set_on_extension_shortcut(&self, callback: Option<ExtensionShortcut>) {
        let old = self.extension.replace(callback);
        drop(old);
    }
    /// Retains one named action without keeping the map borrowed.
    fn action(&self, name: &str) -> Option<Rc<dyn Fn()>> {
        self.action_handlers().borrow().get(name).cloned()
    }
}
impl InputHandler for CustomEditor {
    fn handle_input(&self, data: &str) {
        if let Some(handler) = self.on_extension_shortcut()
            && handler(data)
        {
            return;
        }
        if self.keybindings.matches(data, "app.clipboard.pasteImage") {
            if let Some(handler) = self.on_paste_image() {
                handler();
            }
            return;
        }
        if self.keybindings.matches(data, "app.interrupt") {
            if !self.editor.is_showing_autocomplete()
                && let Some(handler) = self.on_escape().or_else(|| self.action("app.interrupt"))
            {
                handler();
                return;
            }
            self.editor.handle_input(data);
            return;
        }
        if self.keybindings.matches(data, "app.exit") && self.editor.get_text().is_empty() {
            if let Some(handler) = self.on_ctrl_d().or_else(|| self.action("app.exit")) {
                handler();
            }
            return;
        }
        let handlers = self.action_handlers();
        let handler = handlers
            .borrow()
            .iter()
            .filter(|(action, _)| !["app.interrupt", "app.exit"].contains(&action.as_str()))
            .find(|(action, _)| self.keybindings.matches(data, action))
            .map(|(_, handler)| Rc::clone(handler));
        if let Some(handler) = handler {
            handler();
        } else {
            self.editor.handle_input(data);
        }
    }
}
impl Deref for CustomEditor {
    type Target = Editor;
    fn deref(&self) -> &Editor {
        &self.editor
    }
}
impl Focusable for CustomEditor {
    fn focus_flag(&self) -> &FocusFlag {
        self.editor.focus_flag()
    }
}
impl Component for CustomEditor {
    fn render(&self, width: usize) -> Vec<String> {
        self.editor.render(width)
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
}
impl EditorComponent for CustomEditor {
    type Signal = Cancellation;

    fn get_text(&self) -> String {
        EditorComponent::get_text(&self.editor)
    }

    fn set_text(&self, text: &str) {
        EditorComponent::set_text(&self.editor, text);
    }

    fn on_submit(&self) -> Option<TextCallback> {
        EditorComponent::on_submit(&self.editor)
    }

    fn set_on_submit(&self, callback: Option<TextCallback>) {
        EditorComponent::set_on_submit(&self.editor, callback);
    }

    fn on_change(&self) -> Option<TextCallback> {
        EditorComponent::on_change(&self.editor)
    }

    fn set_on_change(&self, callback: Option<TextCallback>) {
        EditorComponent::set_on_change(&self.editor, callback);
    }

    fn add_to_history(&self, text: &str) -> Option<()> {
        EditorComponent::add_to_history(&self.editor, text)
    }

    fn insert_text_at_cursor(&self, text: &str) -> Option<()> {
        EditorComponent::insert_text_at_cursor(&self.editor, text)
    }

    fn get_expanded_text(&self) -> Option<String> {
        EditorComponent::get_expanded_text(&self.editor)
    }

    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    ) -> Option<()> {
        EditorComponent::set_autocomplete_provider(&self.editor, provider)
    }

    fn border_color(&self) -> Option<BorderColor> {
        EditorComponent::border_color(&self.editor)
    }

    fn set_border_color(&self, color: BorderColor) -> Option<()> {
        EditorComponent::set_border_color(&self.editor, color)
    }

    fn set_padding_x(&self, padding: f64) -> Option<()> {
        EditorComponent::set_padding_x(&self.editor, padding)
    }

    fn set_autocomplete_max_visible(&self, max_visible: f64) -> Option<()> {
        EditorComponent::set_autocomplete_max_visible(&self.editor, max_visible)
    }
}
impl maestro_app::ExtensionEditor for CustomEditor {
    fn editor(&self) -> &Editor {
        &self.editor
    }
}
impl CustomEditorComponent for CustomEditor {
    fn action_handlers(&self) -> ActionHandlers {
        CustomEditor::action_handlers(self)
    }
    fn on_escape(&self) -> Option<Rc<dyn Fn()>> {
        CustomEditor::on_escape(self)
    }
    fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>) {
        CustomEditor::set_on_escape(self, callback);
    }
    fn on_ctrl_d(&self) -> Option<Rc<dyn Fn()>> {
        CustomEditor::on_ctrl_d(self)
    }
    fn set_on_ctrl_d(&self, callback: Option<Rc<dyn Fn()>>) {
        CustomEditor::set_on_ctrl_d(self, callback);
    }
    fn on_paste_image(&self) -> Option<Rc<dyn Fn()>> {
        CustomEditor::on_paste_image(self)
    }
    fn set_on_paste_image(&self, callback: Option<Rc<dyn Fn()>>) {
        CustomEditor::set_on_paste_image(self, callback);
    }
    fn on_extension_shortcut(&self) -> Option<ExtensionShortcut> {
        CustomEditor::on_extension_shortcut(self)
    }
    fn set_on_extension_shortcut(&self, callback: Option<ExtensionShortcut>) {
        CustomEditor::set_on_extension_shortcut(self, callback);
    }
}
