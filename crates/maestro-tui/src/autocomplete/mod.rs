//! Command and direct-path completion through replaceable host operations.

mod commands;
mod operations;
mod paths;

pub use operations::{AutocompleteOperations, DirectoryEntry, DirectoryEntryKind};

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

/// A callback failure retained by the provider.
pub type CompletionError = Box<dyn std::error::Error>;

/// Produces argument candidates for a command, immediately or after suspending.
pub type ArgumentCompletions = Rc<
    dyn for<'a> Fn(
        &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Option<Vec<AutocompleteItem>>, CompletionError>> + 'a>,
    >,
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
    ///
    /// # Errors
    /// Returns a provider failure, including an argument callback rejection.
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Self::Signal>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>;

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

/// One of the two command input forms.
#[derive(Clone)]
pub enum Command {
    /// A command with optional argument completion.
    SlashCommand(SlashCommand),
    /// A command supplied as a candidate record.
    AutocompleteItem(AutocompleteItem),
}

/// Suggests command names, command arguments and direct filesystem paths.
pub struct CombinedAutocompleteProvider<O: AutocompleteOperations> {
    /// Owned command definitions in caller order.
    commands: Vec<Command>,
    /// Authored base directory for relative completions.
    base_path: String,
    /// Host filesystem and collation effects.
    operations: O,
}

impl<O: AutocompleteOperations> CombinedAutocompleteProvider<O> {
    /// Construct a provider without accessing the host.
    pub fn new(commands: Vec<Command>, base_path: String, operations: O) -> Self {
        Self {
            commands,
            base_path,
            operations,
        }
    }
}

impl<O: AutocompleteOperations> AutocompleteProvider for CombinedAutocompleteProvider<O> {
    type Signal = O::Signal;

    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Self::Signal>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>
    {
        Box::pin(async move {
            let text = before_cursor(lines, cursor);
            if paths::attachment_prefix(text) {
                return Ok(None);
            }
            if options.force != Some(true) && text.starts_with('/') {
                return commands::suggest(&self.commands, text).await;
            }
            let Some(prefix) = paths::extract_prefix(text, options.force == Some(true)) else {
                return Ok(None);
            };
            Ok(paths::suggest(&self.operations, &self.base_path, prefix)
                .ok()
                .flatten())
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        paths::apply(lines, cursor, item, prefix)
    }

    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
    ) -> Option<bool> {
        Some(commands::should_trigger(before_cursor(lines, cursor)))
    }
}

/// Current line prefix, with missing lines treated as empty.
fn before_cursor(lines: &[String], cursor: CursorPosition) -> &str {
    lines
        .get(cursor.line)
        .map_or("", |line| &line[..cursor.col])
}

#[cfg(not(target_arch = "wasm32"))]
pub use operations::NativeAutocompleteOperations;
