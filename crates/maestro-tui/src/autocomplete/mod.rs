//! Contracts between an editor and whatever suggests completions for it.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

/// A position in the lines being edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorPosition {
    /// Zero-based line index.
    pub line: usize,
    /// UTF-8 byte offset within the line, never a cell or scalar count.
    pub col: usize,
}

/// One completion candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutocompleteItem {
    /// Text inserted when the item is chosen.
    pub value: String,
    /// Text shown in the list.
    pub label: String,
    /// Optional explanation shown beside the label.
    pub description: Option<String>,
}

/// Candidates together with the text they replace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutocompleteSuggestions {
    /// Candidates; an empty list differs from having no suggestions at all.
    pub items: Vec<AutocompleteItem>,
    /// The text being matched, such as `/` or `src/`.
    pub prefix: String,
}

/// What a request for suggestions carries besides the text and cursor.
pub struct CompletionOptions<'a, S: ?Sized> {
    /// The caller's cancellation signal, borrowed for the request.
    pub signal: &'a S,
    /// Whether the user forced completion; absent when unspecified.
    pub force: Option<bool>,
}

/// The lines and cursor after a completion was applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionResult {
    /// The edited lines.
    pub lines: Vec<String>,
    /// Zero-based line of the cursor.
    pub cursor_line: usize,
    /// UTF-8 byte offset of the cursor within its line.
    pub cursor_col: usize,
}

/// Produces argument candidates for a command, immediately or after suspending.
pub type ArgumentCompletions = Rc<
    dyn for<'a> Fn(&'a str) -> Pin<Box<dyn Future<Output = Option<Vec<AutocompleteItem>>> + 'a>>,
>;

/// A slash command offered for completion.
#[derive(Clone)]
pub struct SlashCommand {
    /// Command name without the slash.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Optional hint describing the arguments.
    pub argument_hint: Option<String>,
    /// Argument completions; `None` when the command has none.
    pub get_argument_completions: Option<ArgumentCompletions>,
}

/// Supplies completions for text being edited.
pub trait AutocompleteProvider {
    /// The caller's cancellation signal type, borrowed per request.
    type Signal: ?Sized;

    /// Suggestions for the text and cursor, or `None` when there are none.
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Self::Signal>,
    ) -> Pin<Box<dyn Future<Output = Option<AutocompleteSuggestions>> + 'a>>;

    /// Applies a chosen item in place of `prefix` and returns the new text and cursor.
    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult;

    /// Whether explicit Tab completion should offer file paths; `None` when the provider
    /// has no opinion.
    fn should_trigger_file_completion(
        &self,
        _lines: &[String],
        _cursor: CursorPosition,
    ) -> Option<bool> {
        None
    }
}
