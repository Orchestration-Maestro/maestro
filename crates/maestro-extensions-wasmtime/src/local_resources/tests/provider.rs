//! A lent completion provider ends with its capability.
use super::ended_endpoint;
use crate::{guest_calls::providers::ProviderEndpoint, local_resources::Capability};
use maestro_extensions::{
    AutocompleteItem, Cancellation,
    presentation::{
        AutocompleteProvider, AutocompleteSuggestions, CompletionError, CompletionOptions,
        CompletionResult, CursorPosition,
    },
};
use std::{future::Future, pin::Pin, rc::Rc};

/// A provider that suggests nothing and applies nothing.
struct Silent;

impl AutocompleteProvider for Silent {
    type Signal = Cancellation;

    fn get_suggestions<'a>(
        &'a self,
        _lines: &'a [String],
        _cursor: CursorPosition,
        _options: CompletionOptions<'a, Cancellation>,
    ) -> Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>
    {
        Box::pin(async { Ok(None) })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor: CursorPosition,
        _item: &AutocompleteItem,
        _prefix: &str,
    ) -> CompletionResult {
        CompletionResult {
            lines: lines.to_vec(),
            cursor_line: cursor.line,
            cursor_col: cursor.col,
        }
    }
}

#[test]
fn released_completion_provider_rejects_every_kind_of_operation_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::Provider(Rc::new(Silent)),
        ProviderEndpoint,
        |provider| {
            let item = AutocompleteItem {
                value: "v".into(),
                label: "l".into(),
                description: None,
            };
            [
                provider
                    .apply(Vec::new(), (0, 0), item, String::new())
                    .result()
                    .unwrap_err(),
                provider
                    .should_trigger_file_completion(Vec::new(), (0, 0))
                    .result()
                    .unwrap_err(),
            ]
        },
    );
    assert_eq!(errors, ["extension completion provider resource ended"; 2]);
}
