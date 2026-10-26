//! The host's toolkit editor and the application's custom editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::widgets::{Widget, host};
use super::{
    Component, ComponentView, EditorTheme, Focusable, KeybindingsManager, Style, TUI, WidgetNotice,
};
use crate::{
    AutocompleteProvider, CallbackEmitter, CustomEditorComponent, EditorComponent, ExtensionResult,
};
use std::ops::Deref;
use std::rc::Rc;

/// Receives an editor's submitted or changed text.
pub type TextHandler = Rc<dyn for<'a> Fn(&str, CallbackEmitter<'a>) -> ExtensionResult<()>>;
/// Receives input before a custom editor and answers whether it consumed it.
pub type EditorShortcut = Rc<dyn for<'a> Fn(&str, CallbackEmitter<'a>) -> ExtensionResult<bool>>;

/// Editor options; an absent member keeps the editor's default.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EditorOptions {
    /// Columns of padding on each side.
    pub padding_x: Option<f64>,
    /// Rows the completion list shows.
    pub autocomplete_max_visible: Option<f64>,
}

port! {
    /// Requests to the host editor behind a facade.
    EditorPort for Editor via editor {
        /// The text.
        fn get_text() -> String;
        /// Replaces the text; the change handler runs first.
        fn set_text(text: &str) -> ();
        /// The text with pasted blocks expanded.
        fn get_expanded_text() -> String;
        /// The lines.
        fn get_lines() -> Vec<String>;
        /// The cursor's line and column.
        fn get_cursor() -> (u32, u32);
        /// Inserts text at the cursor; the change handler runs first.
        fn insert_text_at_cursor(text: &str) -> ();
        /// Adds text to the history.
        fn add_to_history(text: &str) -> ();
        /// The side padding.
        fn get_padding_x() -> f64;
        /// Replaces the side padding.
        fn set_padding_x(padding: f64) -> ();
        /// The rows the completion list shows.
        fn get_autocomplete_max_visible() -> f64;
        /// Replaces the rows the completion list shows.
        fn set_autocomplete_max_visible(max_visible: f64) -> ();
        /// Whether submission is disabled.
        fn disable_submit() -> bool;
        /// Disables or enables submission.
        fn set_disable_submit(disabled: bool) -> ();
        /// Whether the completion list shows.
        fn is_showing_autocomplete() -> bool;
        /// Whether the editor has the focus.
        fn focused() -> bool;
        /// Gives or takes the focus.
        fn set_focused(focused: bool) -> ();
        /// The border style.
        fn border_color() -> Style;
        /// Replaces the border style.
        fn set_border_color(color: &Style) -> ();
        /// Replaces or removes the submission handler.
        fn set_on_submit(handler: Option<TextHandler>) -> ();
        /// The change handler last set through this editor, if any.
        fn on_change() -> Option<TextHandler>;
        /// Replaces or removes the change handler.
        fn set_on_change(handler: Option<TextHandler>) -> ();
        /// Delivers input; the editor's handlers may run first.
        fn handle_input(data: &str) -> ();
        /// Replaces the completion provider.
        fn set_autocomplete_provider(provider: Rc<dyn AutocompleteProvider>) -> ();
    }
}

port! {
    /// Requests to the host custom editor behind a facade.
    CustomEditorPort for CustomEditor via custom {
        /// Replaces the handler of an application action, keeping its place.
        fn on_action(action: &str, handler: WidgetNotice) -> ();
        /// Replaces or removes the interrupt handler.
        fn set_on_escape(handler: Option<WidgetNotice>) -> ();
        /// Replaces or removes the empty-editor exit handler.
        fn set_on_ctrl_d(handler: Option<WidgetNotice>) -> ();
        /// Replaces or removes the image-paste handler.
        fn set_on_paste_image(handler: Option<WidgetNotice>) -> ();
        /// Replaces or removes the interceptor that sees input first.
        fn set_on_extension_shortcut(handler: Option<EditorShortcut>) -> ();
    }
}

/// The host's multiline editor.
#[derive(Clone)]
pub struct Editor {
    /// The widget.
    widget: Widget,
    /// The editor requests of the same widget.
    editor: Rc<dyn EditorPort>,
}

impl Editor {
    /// Builds the editor on a lent terminal.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(tui: &TUI, theme: &EditorTheme, options: EditorOptions) -> ExtensionResult<Self> {
        let (widget, editor) = host()?.editor(tui, theme, options)?;
        Ok(Self { widget, editor })
    }
    /// Renders rows at `width`; the host may call authored functions first.
    ///
    /// # Errors
    /// Returns the host's or an authored function's failure.
    pub fn render(&self, width: u32) -> ExtensionResult<Vec<String>> {
        self.widget.port.render(width)
    }
    /// Drops cached rows.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn invalidate(&self) -> ExtensionResult<()> {
        self.widget.port.invalidate()
    }
    /// An editor over a widget and its editor requests.
    pub(crate) fn hosted(widget: Widget, editor: Rc<dyn EditorPort>) -> Self {
        Self { widget, editor }
    }
    /// The host widget behind this editor.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn widget(&self) -> &Widget {
        &self.widget
    }
}

impl Focusable for Editor {
    fn focused(&self, _: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Editor::focused(self)
    }
    fn set_focused(&self, focused: bool, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Editor::set_focused(self, focused)
    }
}

impl ComponentView for Editor {
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Editor::render(self, width)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Editor::handle_input(self, data)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Editor::invalidate(self)
    }
}

/// The host editor supports every optional editor operation.
impl EditorComponent for Editor {
    fn get_text(&self, _: CallbackEmitter<'_>) -> ExtensionResult<String> {
        Editor::get_text(self)
    }
    fn set_text(&self, text: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Editor::set_text(self, text)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Editor::handle_input(self, data)
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        Editor::set_on_submit(self, handler)
    }
    fn on_change(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        Editor::on_change(self)
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        Editor::set_on_change(self, handler)
    }
    fn add_to_history(&self, text: &str, _: CallbackEmitter<'_>) -> ExtensionResult<Option<()>> {
        Editor::add_to_history(self, text).map(Some)
    }
    fn insert_text_at_cursor(
        &self,
        text: &str,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Editor::insert_text_at_cursor(self, text).map(Some)
    }
    fn get_expanded_text(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<String>> {
        Editor::get_expanded_text(self).map(Some)
    }
    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Editor::set_autocomplete_provider(self, provider).map(Some)
    }
    fn border_color(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<Style>> {
        Editor::border_color(self).map(Some)
    }
    fn set_border_color(
        &self,
        color: Style,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Editor::set_border_color(self, &color).map(Some)
    }
    fn set_padding_x(&self, padding: f64, _: CallbackEmitter<'_>) -> ExtensionResult<Option<()>> {
        Editor::set_padding_x(self, padding).map(Some)
    }
    fn set_autocomplete_max_visible(
        &self,
        max_visible: f64,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Editor::set_autocomplete_max_visible(self, max_visible).map(Some)
    }
}

impl From<&Editor> for Component {
    fn from(editor: &Editor) -> Self {
        Component::hosted(editor.clone(), editor.widget.clone())
    }
}

/// The application's editor: the host editor with application actions, special keys and a
/// shortcut interceptor.
#[derive(Clone)]
pub struct CustomEditor {
    /// The editor operations.
    editor: Editor,
    /// The custom editor requests of the same widget.
    custom: Rc<dyn CustomEditorPort>,
}

impl CustomEditor {
    /// Builds the custom editor on a lent terminal with a keybindings manager.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn new(
        tui: &TUI,
        theme: &EditorTheme,
        keybindings: &KeybindingsManager,
        options: EditorOptions,
    ) -> ExtensionResult<Self> {
        host()?
            .custom_editor(tui, theme, keybindings, options)
            .map(Self::hosted)
    }
    /// A custom editor over a widget and its editor and custom editor requests.
    pub(crate) fn hosted(
        (widget, editor, custom): (Widget, Rc<dyn EditorPort>, Rc<dyn CustomEditorPort>),
    ) -> Self {
        Self {
            editor: Editor::hosted(widget, editor),
            custom,
        }
    }
}

impl CustomEditor {
    /// Renders rows at `width`; the host may call authored functions first.
    ///
    /// # Errors
    /// Returns the host's or an authored function's failure.
    pub fn render(&self, width: u32) -> ExtensionResult<Vec<String>> {
        self.editor.render(width)
    }
    /// Drops cached rows.
    ///
    /// # Errors
    /// Returns the host's failure.
    pub fn invalidate(&self) -> ExtensionResult<()> {
        self.editor.invalidate()
    }
    /// Delivers input; the editor's handlers may run first.
    ///
    /// # Errors
    /// Returns the host's or a handler's failure.
    pub fn handle_input(&self, data: &str) -> ExtensionResult<()> {
        self.editor.handle_input(data)
    }
}

impl Deref for CustomEditor {
    type Target = Editor;
    fn deref(&self) -> &Editor {
        &self.editor
    }
}

impl Focusable for CustomEditor {
    fn focused(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Focusable::focused(&self.editor, emitter)
    }
    fn set_focused(&self, focused: bool, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Focusable::set_focused(&self.editor, focused, emitter)
    }
}

impl ComponentView for CustomEditor {
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        ComponentView::render(&self.editor, width, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::handle_input(&self.editor, data, emitter)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::invalidate(&self.editor, emitter)
    }
}

/// Delegates every editor operation to the host editor it extends.
impl EditorComponent for CustomEditor {
    fn get_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<String> {
        EditorComponent::get_text(&self.editor, emitter)
    }
    fn set_text(&self, text: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::set_text(&self.editor, text, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::handle_input(&self.editor, data, emitter)
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        EditorComponent::set_on_submit(&self.editor, handler, emitter)
    }
    fn on_change(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        EditorComponent::on_change(&self.editor, emitter)
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        EditorComponent::set_on_change(&self.editor, handler, emitter)
    }
    fn add_to_history(
        &self,
        text: &str,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::add_to_history(&self.editor, text, emitter)
    }
    fn insert_text_at_cursor(
        &self,
        text: &str,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::insert_text_at_cursor(&self.editor, text, emitter)
    }
    fn get_expanded_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<String>> {
        EditorComponent::get_expanded_text(&self.editor, emitter)
    }
    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_autocomplete_provider(&self.editor, provider, emitter)
    }
    fn border_color(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<Style>> {
        EditorComponent::border_color(&self.editor, emitter)
    }
    fn set_border_color(
        &self,
        color: Style,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_border_color(&self.editor, color, emitter)
    }
    fn set_padding_x(
        &self,
        padding: f64,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_padding_x(&self.editor, padding, emitter)
    }
    fn set_autocomplete_max_visible(
        &self,
        max_visible: f64,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_autocomplete_max_visible(&self.editor, max_visible, emitter)
    }
}

impl CustomEditorComponent for CustomEditor {
    fn custom_editor(&self) -> &CustomEditor {
        self
    }
}

impl From<&CustomEditor> for Component {
    fn from(editor: &CustomEditor) -> Self {
        Component::from(&editor.editor)
    }
}
