//! Controlled completion providers with two unrelated cancellation signal types.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;

use maestro_tui::autocomplete::{CompletionOptions, CompletionResult, CursorPosition};
use maestro_tui::{AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions};

use super::futures::YieldOnce;

/// A candidate whose value and label are the same text.
pub fn item(value: &str) -> AutocompleteItem {
    AutocompleteItem {
        value: value.to_owned(),
        label: value.to_owned(),
        description: None,
    }
}

/// Suggests commands and stops early when its flag signal is raised.
pub struct CommandProvider;

impl AutocompleteProvider for CommandProvider {
    type Signal = Cell<bool>;

    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a, Self::Signal>,
    ) -> Pin<Box<dyn Future<Output = Option<AutocompleteSuggestions>> + 'a>> {
        Box::pin(async move {
            YieldOnce(false).await;
            if options.signal.get() {
                return None;
            }
            let before = &lines[cursor.line][..cursor.col];
            let prefix = before.rsplit(' ').next().unwrap_or_default();
            let mut items: Vec<_> = ["/help", "/hello"]
                .into_iter()
                .filter(|command| command.starts_with(prefix))
                .map(item)
                .collect();
            if options.force == Some(true) {
                items.push(item("/forced"));
            }
            Some(AutocompleteSuggestions {
                items,
                prefix: prefix.to_owned(),
            })
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        chosen: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        let line = &lines[cursor.line];
        let start = cursor.col - prefix.len();
        let mut updated = lines.to_vec();
        updated[cursor.line] = format!("{}{}{}", &line[..start], chosen.value, &line[cursor.col..]);
        CompletionResult {
            lines: updated,
            cursor_line: cursor.line,
            cursor_col: start + chosen.value.len(),
        }
    }

    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
    ) -> Option<bool> {
        Some(lines[cursor.line][..cursor.col].contains('/'))
    }
}

/// Counts requests in a string signal and tells "nothing" from "no match".
pub struct CountingProvider;

impl AutocompleteProvider for CountingProvider {
    type Signal = RefCell<String>;

    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        _cursor: CursorPosition,
        options: CompletionOptions<'a, Self::Signal>,
    ) -> Pin<Box<dyn Future<Output = Option<AutocompleteSuggestions>> + 'a>> {
        Box::pin(async move {
            options.signal.borrow_mut().push_str("seen;");
            match lines.first().map(String::as_str) {
                Some("") | None => None,
                Some("none") => Some(AutocompleteSuggestions {
                    items: Vec::new(),
                    prefix: "none".into(),
                }),
                Some(text) => Some(AutocompleteSuggestions {
                    items: vec![item(text)],
                    prefix: text.into(),
                }),
            }
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        _chosen: &AutocompleteItem,
        _prefix: &str,
    ) -> CompletionResult {
        CompletionResult {
            lines: lines.to_vec(),
            cursor_line: cursor.line,
            cursor_col: cursor.col,
        }
    }
}
