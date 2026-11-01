//! Completion providers and the candidates they offer to an editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
pub use crate::bindings::maestro::extension::host::AutocompleteItem;
use crate::{AbortSignal, CallbackEmitter, ExtensionFuture, ExtensionResult};
use std::rc::Rc;

/// A cursor position in the edited lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPosition {
    /// Zero-based line index.
    pub line: u32,
    /// UTF-8 byte offset within the line.
    pub col: u32,
}

/// Candidates together with the text they replace.
#[derive(Clone, Debug)]
pub struct AutocompleteSuggestions {
    /// Candidates; an empty list differs from no suggestions.
    pub items: Vec<AutocompleteItem>,
    /// The text being matched, such as `/` or `src/`.
    pub prefix: String,
}

/// Caller options for one asynchronous suggestion request.
pub struct CompletionOptions<'a> {
    /// The caller's cancellation signal; clone it to retain it independently.
    pub signal: &'a AbortSignal,
    /// Whether the user forced completion; absent when unspecified.
    pub force: Option<bool>,
}

/// The lines and cursor after a completion was applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionResult {
    /// The edited lines.
    pub lines: Vec<String>,
    /// Zero-based line index.
    pub cursor_line: u32,
    /// UTF-8 byte offset within the line.
    pub cursor_col: u32,
}

/// Supplies completions for text being edited.
///
/// A provider may retain a previous provider to wrap its behavior. Suggestion futures may
/// suspend; completion application and file-trigger observations remain synchronous.
pub trait AutocompleteProvider {
    /// Suggestions for the text and cursor, or `None` when there are none.
    ///
    /// # Errors
    /// Returns the provider's failure without converting it to an empty result.
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a>,
    ) -> ExtensionFuture<'a, Option<AutocompleteSuggestions>>;

    /// Applies a chosen item in place of `prefix` and returns the edited text and cursor.
    ///
    /// # Errors
    /// Returns the provider's failure.
    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<CompletionResult>;

    /// Whether explicit Tab completion should offer files; `None` means no opinion.
    ///
    /// # Errors
    /// Returns the provider's failure.
    fn should_trigger_file_completion(
        &self,
        _lines: &[String],
        _cursor: CursorPosition,
    ) -> ExtensionResult<Option<bool>> {
        Ok(None)
    }
}

/// Wraps the current completion provider with additional behavior, synchronously.
pub type AutocompleteProviderFactory = Rc<
    dyn for<'a> Fn(
        Rc<dyn AutocompleteProvider>,
        CallbackEmitter<'a>,
    ) -> ExtensionResult<Rc<dyn AutocompleteProvider>>,
>;
