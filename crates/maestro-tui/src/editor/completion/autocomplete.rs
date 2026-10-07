//! Caller-supplied completion contracts.
use std::{future::Future, pin::Pin, rc::Rc};
/// A suggested insertion with display metadata.
pub struct AutocompleteItem {
    /// Inserted value.
    pub value: String,
    /// Display label.
    pub label: String,
    /// Optional display description.
    pub description: Option<String>,
}
/// Command metadata with optional argument suggestions.
#[allow(clippy::type_complexity)]
pub struct SlashCommand {
    /// Command name.
    pub name: String,
    /// Optional display description.
    pub description: Option<String>,
    /// Optional argument hint.
    pub argument_hint: Option<String>,
    /// Optional callback; run its prefix before returning the continuation.
    pub get_argument_completions:
        Option<Rc<dyn Fn(String) -> Pin<Box<dyn Future<Output = Option<Vec<AutocompleteItem>>>>>>>,
}
/// Ordered suggestions with their matched prefix.
pub struct AutocompleteSuggestions {
    /// Ordered completion items.
    pub items: Vec<AutocompleteItem>,
    /// Matched prefix.
    pub prefix: String,
}
/// Caller-supplied abort state and listeners.
pub trait AbortSignal {
    /// Aborted.
    fn aborted(&self) -> bool;
    /// Register an abort listener by shared identity, optionally once.
    fn add_event_listener(&self, listener: Rc<dyn Fn()>, once: bool);
    /// Remove the listener with this shared identity.
    fn remove_event_listener(&self, listener: &Rc<dyn Fn()>);
}
/// Caller-supplied suggestion and insertion behavior.
#[allow(clippy::type_complexity)]
pub trait AutocompleteProvider {
    /// Start suggestions with a UTF-8 byte-offset cursor position, signal and optional force.
    /// The synchronous prefix runs before returning an owned continuation.
    fn get_suggestions(
        &mut self,
        lines: Vec<String>,
        cursor_line: usize,
        cursor_col: usize,
        options: (Rc<dyn AbortSignal>, Option<bool>),
    ) -> Pin<Box<dyn Future<Output = Option<AutocompleteSuggestions>>>>;
    /// Insert a completion, returning lines and line/UTF-8 byte offset.
    fn apply_completion(
        &mut self,
        lines: Vec<String>,
        cursor_line: usize,
        cursor_col: usize,
        item: AutocompleteItem,
        prefix: &str,
    ) -> (Vec<String>, usize, usize);
    /// Optional file-completion trigger capability; defaults to absent.
    fn should_trigger_file_completion(
        &mut self,
    ) -> Option<Box<dyn FnMut(&[String], usize, usize) -> bool + '_>> {
        None
    }
}
