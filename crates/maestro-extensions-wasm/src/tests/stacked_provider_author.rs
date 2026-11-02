//! A completion provider stacked on the host's current provider.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions,
    CompletionOptions, CompletionResult, CursorPosition, ExtensionFuture, ExtensionResult,
    ExtensionUIContext,
};
use serde_json::json;

/// A wrapper that adds its own candidate to the current provider's suggestions, answers the
/// line `mine` itself and passes everything else on. It keeps the last signal it received;
/// the decision for the line `kept` reports whether that signal was cancelled.
struct Wrapper {
    /// The candidate this wrapper adds.
    name: String,
    /// The provider it wraps.
    current: Rc<dyn AutocompleteProvider>,
    /// The signal of the last suggestion request.
    kept: RefCell<Option<AbortSignal>>,
}

impl Wrapper {
    /// This wrapper's own candidate.
    fn item(&self) -> AutocompleteItem {
        AutocompleteItem {
            value: self.name.clone(),
            label: self.name.to_uppercase(),
            description: None,
        }
    }
}

impl AutocompleteProvider for Wrapper {
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a>,
    ) -> ExtensionFuture<'a, Option<AutocompleteSuggestions>> {
        self.kept.replace(Some(options.signal.clone()));
        Box::pin(async move {
            if lines.first().is_some_and(|line| line == "mine") {
                return Ok(Some(AutocompleteSuggestions {
                    items: vec![self.item()],
                    prefix: "mine".to_owned(),
                }));
            }
            let found = self.current.get_suggestions(lines, cursor, options).await?;
            Ok(found.map(|mut found| {
                found.items.push(self.item());
                found
            }))
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<CompletionResult> {
        if item.value != self.name {
            return self.current.apply_completion(lines, cursor, item, prefix);
        }
        Ok(CompletionResult {
            lines: vec![format!("{prefix}->{}", self.name)],
            cursor_line: cursor.line,
            cursor_col: cursor.col + 1,
        })
    }

    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
    ) -> ExtensionResult<Option<bool>> {
        if lines.first().is_some_and(|line| line == "kept") {
            return Ok(self.kept.borrow().as_ref().map(AbortSignal::aborted));
        }
        self.current.should_trigger_file_completion(lines, cursor)
    }
}

/// Stacks a wrapper named `name` on the current provider.
pub fn install(ui: &ExtensionUIContext, name: &str) -> ExtensionResult<()> {
    let name = name.to_owned();
    ui.add_autocomplete_provider(Rc::new(move |current, emitter| {
        emitter.emit("provider-factory", json!(name))?;
        Ok(Rc::new(Wrapper {
            name: name.clone(),
            current,
            kept: RefCell::default(),
        }))
    }))
}
