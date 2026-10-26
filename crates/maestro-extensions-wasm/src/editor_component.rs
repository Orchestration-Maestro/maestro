//! The synchronous operations of an authored replacement editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use crate::{
    AutocompleteProvider, CallbackEmitter, ComponentView, CustomEditor, EditorTheme,
    ExtensionResult, KeybindingsManager, Style, TUI, TextHandler,
};

/// An authored text editor with independently optional advanced operations.
///
/// Rendering and invalidation use [`ComponentView`]. Input uses this trait's required
/// [`handle_input`](Self::handle_input) operation, not the component's default input handler.
/// A component input handler can delegate to that operation with a qualified call.
///
/// Optional operations return `None` when absent, not a successful no-op. Callers that need
/// expanded text fall back to [`get_text`](Self::get_text) when expansion is absent.
/// Every invocation receives an emitter borrowed only for that callback.
pub trait EditorComponent: ComponentView {
    /// Reads the current text.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn get_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<String>;

    /// Replaces the text, running the change handler synchronously when present.
    ///
    /// # Errors
    /// Returns the editor's or handler's failure.
    fn set_text(&self, text: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;

    /// Handles literal terminal input synchronously.
    ///
    /// # Errors
    /// Returns the editor's or handler's failure.
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()>;

    /// Replaces or removes the submission callback.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()>;

    /// Reads the change callback, when one is installed.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn on_change(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>>;

    /// Replaces or removes the change callback.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()>;

    /// Adds text to the history, when supported.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn add_to_history(
        &self,
        _text: &str,
        _emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Ok(None)
    }

    /// Inserts text at the cursor, when supported.
    ///
    /// # Errors
    /// Returns the editor's or handler's failure.
    fn insert_text_at_cursor(
        &self,
        _text: &str,
        _emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Ok(None)
    }

    /// Reads text with markers expanded, or `None` when expansion is absent.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn get_expanded_text(&self, _emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<String>> {
        Ok(None)
    }

    /// Installs a completion provider, when supported.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn set_autocomplete_provider(
        &self,
        _provider: Rc<dyn AutocompleteProvider>,
        _emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Ok(None)
    }

    /// Reads the border painter, or `None` when there is none to share.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn border_color(&self, _emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<Style>> {
        Ok(None)
    }

    /// Changes the border painter, when supported.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn set_border_color(
        &self,
        _color: Style,
        _emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Ok(None)
    }

    /// Changes the side padding, when supported.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn set_padding_x(
        &self,
        _padding: f64,
        _emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Ok(None)
    }

    /// Changes the completion row count, when supported.
    ///
    /// # Errors
    /// Returns the editor's failure.
    fn set_autocomplete_max_visible(
        &self,
        _max_visible: f64,
        _emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        Ok(None)
    }
}

/// An authored editor built on the application's custom editor, whose application actions
/// and special keys the host shares with it.
///
/// The host reaches the editor operations through [`EditorComponent`], and the actions and
/// interrupt, exit, image-paste and shortcut callbacks through the custom editor this editor
/// names. Input the editor passes on to that custom editor runs those actions.
pub trait CustomEditorComponent: EditorComponent {
    /// The application's custom editor this editor extends.
    fn custom_editor(&self) -> &CustomEditor;
}

/// The editor an editor factory built: a plain editor, or one with the application's actions.
#[derive(Clone)]
pub enum EditorFactoryOutcome {
    /// An editor with only the editor operations.
    Editor(Rc<dyn EditorComponent>),
    /// An editor that also shares the application's actions and callbacks.
    CustomEditor(Rc<dyn CustomEditorComponent>),
}

impl EditorFactoryOutcome {
    /// The built editor through its editor operations.
    #[must_use]
    pub fn editor(&self) -> Rc<dyn EditorComponent> {
        match self {
            Self::Editor(editor) => Rc::clone(editor),
            Self::CustomEditor(editor) => Rc::clone(editor) as Rc<dyn EditorComponent>,
        }
    }
}

/// Builds a replacement editor from the terminal, the editor styles and the keybindings,
/// synchronously.
pub type EditorFactory = Rc<
    dyn for<'a> Fn(
        TUI,
        EditorTheme,
        KeybindingsManager,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<EditorFactoryOutcome>,
>;
