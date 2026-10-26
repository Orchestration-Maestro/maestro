//! The host's toolkit editor and the application's custom editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::widgets::{Hosted, Widget, host};
use super::{Component, EditorTheme, KeybindingsManager, Style, TUI, WidgetNotice};
use crate::{CallbackEmitter, ExtensionResult};
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
        /// Replaces the border style.
        fn set_border_color(color: &Style) -> ();
        /// Replaces or removes the submission handler.
        fn set_on_submit(handler: Option<TextHandler>) -> ();
        /// Replaces or removes the change handler.
        fn set_on_change(handler: Option<TextHandler>) -> ();
        /// Delivers input; the editor's handlers may run first.
        fn handle_input(data: &str) -> ();
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
}

impl From<&Editor> for Component {
    fn from(editor: &Editor) -> Self {
        Component::hosted(Hosted(editor.widget.clone()), editor.widget.clone())
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
        let (widget, editor, custom) = host()?.custom_editor(tui, theme, keybindings, options)?;
        Ok(Self {
            editor: Editor::hosted(widget, editor),
            custom,
        })
    }
}

impl Deref for CustomEditor {
    type Target = Editor;
    fn deref(&self) -> &Editor {
        &self.editor
    }
}

impl From<&CustomEditor> for Component {
    fn from(editor: &CustomEditor) -> Self {
        Component::from(&editor.editor)
    }
}
