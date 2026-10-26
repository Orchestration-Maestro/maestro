//! The synchronous operations of an authored replacement editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use crate::{
    AutocompleteProvider, CallbackEmitter, ComponentView, ExtensionResult, Style, TextHandler,
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
