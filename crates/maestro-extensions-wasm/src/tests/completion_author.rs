//! An authored completion provider whose asynchronous work uses the retained host UI.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions,
    CompletionOptions, CompletionResult, CursorPosition, ExtensionFuture, ExtensionResult,
    ExtensionUIContext, ExtensionUIDialogOptions,
};
use serde_json::{Value, json};

/// Completion requests go to this independently retained UI.
struct Provider(ExtensionUIContext);

impl AutocompleteProvider for Provider {
    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        cursor: CursorPosition,
        options: CompletionOptions<'a>,
    ) -> ExtensionFuture<'a, Option<AutocompleteSuggestions>> {
        let signal = options.signal.clone();
        Box::pin(async move {
            let title = json!([lines, cursor.line, cursor.col, options.force]).to_string();
            let opts = ExtensionUIDialogOptions {
                signal: Some(signal),
                timeout: None,
            };
            Ok(self
                .0
                .input(title, None, Some(opts))
                .await?
                .map(|answer| AutocompleteSuggestions {
                    items: if answer.is_empty() {
                        Vec::new()
                    } else {
                        vec![AutocompleteItem {
                            value: answer,
                            label: "snow 雪".to_owned(),
                            description: Some(String::new()),
                        }]
                    },
                    prefix: "λ/".to_owned(),
                }))
        })
    }

    fn apply_completion(
        &self,
        _lines: &[String],
        cursor: CursorPosition,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<CompletionResult> {
        Ok(CompletionResult {
            lines: vec![prefix.to_owned(), item.value.clone()],
            cursor_line: cursor.line,
            cursor_col: cursor.col,
        })
    }
}

/// Retains a completion provider that delegates its pending work to `ui`.
pub fn provider(ui: ExtensionUIContext) -> Rc<dyn AutocompleteProvider> {
    Rc::new(Provider(ui))
}

/// Calls a provider through its public interface and observes optional results independently.
pub async fn run(
    ui: ExtensionUIContext,
    signal: AbortSignal,
    force: Option<bool>,
) -> ExtensionResult<Value> {
    let provider = provider(ui);
    let lines = vec!["λ/".to_owned(), String::new()];
    let cursor = CursorPosition { line: 1, col: 0 };
    let file = provider.should_trigger_file_completion(&lines, cursor)?;
    let suggestions = provider
        .get_suggestions(
            &lines,
            cursor,
            CompletionOptions {
                signal: &signal,
                force,
            },
        )
        .await?;
    let Some(suggestions) = suggestions else {
        return Ok(json!([file, null]));
    };
    let applied = suggestions
        .items
        .first()
        .map(|item| provider.apply_completion(&lines, cursor, item, &suggestions.prefix))
        .transpose()?;
    Ok(json!([
        file,
        suggestions
            .items
            .iter()
            .map(|item| json!([item.value, item.label, item.description]))
            .collect::<Vec<_>>(),
        suggestions.prefix,
        applied.map(|result| json!([result.lines, result.cursor_line, result.cursor_col]))
    ]))
}
