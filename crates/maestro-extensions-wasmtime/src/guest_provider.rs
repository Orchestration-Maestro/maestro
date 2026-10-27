//! A native completion provider over an authored provider, reached through its original Store.
use crate::guest_calls::{
    Call, Handle,
    paints::Failure,
    providers::{Answer, Completed, ProviderCall, Request, Suggestions, cursor},
    widget_calls::Item,
};
use crate::local_resources::Registry;
use maestro_extensions::widgets::{
    AutocompleteProvider, AutocompleteSuggestions, CompletionError, CompletionOptions,
    CompletionResult, CursorPosition,
};
use maestro_extensions::{AutocompleteItem, Cancellation};
use std::{future::Future, pin::Pin};

/// The native side of an authored completion provider.
pub(crate) struct GuestProvider {
    /// The provider's owner.
    owner: Handle,
    /// Original capability registry.
    registry: Registry,
    /// The first failure of an operation whose signature cannot return it.
    failure: Failure,
}

impl GuestProvider {
    /// Retains the provider a factory returned.
    pub(crate) fn new(owner: Handle, registry: Registry) -> Self {
        Self {
            owner,
            registry,
            failure: Failure::default(),
        }
    }

    /// Runs a request that answers at once.
    fn synchronous(&self, request: Request) -> Result<Answer, String> {
        self.owner.endpoint().synchronous(&self.registry, |reply| {
            Ok(Call::Autocomplete(ProviderCall {
                handler: self.owner.clone(),
                request,
                signal: None,
                reply,
            }))
        })
    }

    /// Keeps a failure the native signature cannot return, for the next suggestion request.
    fn keep(&self, error: String) {
        self.failure.borrow_mut().get_or_insert(error);
    }
}

/// The canonical form of a native candidate.
fn item(item: &AutocompleteItem) -> Item {
    Item::new(&item.value, &item.label, item.description.as_ref())
}

/// The native form of canonical suggestions.
fn suggestions(suggestions: Suggestions) -> AutocompleteSuggestions {
    AutocompleteSuggestions {
        items: suggestions
            .items
            .into_iter()
            .map(|item| AutocompleteItem {
                value: item.value,
                label: item.label,
                description: item.description,
            })
            .collect(),
        prefix: suggestions.prefix,
    }
}

/// The native form of a canonical completion.
fn completed(completed: Completed) -> Result<CompletionResult, String> {
    let convert = |value: u32| usize::try_from(value).map_err(|error| error.to_string());
    Ok(CompletionResult {
        cursor_line: convert(completed.cursor_line)?,
        cursor_col: convert(completed.cursor_col)?,
        lines: completed.lines,
    })
}

/// The suggestion future of a native provider.
type Pending<'a> =
    Pin<Box<dyn Future<Output = Result<Option<AutocompleteSuggestions>, CompletionError>> + 'a>>;

impl AutocompleteProvider for GuestProvider {
    type Signal = Cancellation;

    fn get_suggestions<'a>(
        &'a self,
        lines: &'a [String],
        at: CursorPosition,
        options: CompletionOptions<'a, Cancellation>,
    ) -> Pending<'a> {
        Box::pin(async move {
            if let Some(error) = self.failure.borrow_mut().take() {
                return Err(error.into());
            }
            let request = Request::Suggestions((lines.to_vec(), cursor(at)?, options.force));
            let signal = self.registry.signal(options.signal.clone())?;
            let answer = self
                .owner
                .endpoint()
                .call(&self.registry, |reply| {
                    Call::Autocomplete(ProviderCall {
                        handler: self.owner.clone(),
                        request,
                        signal: Some(signal),
                        reply,
                    })
                })
                .await?;
            match answer {
                Answer::Suggestions(found) => Ok(found.map(suggestions)),
                _ => Err("the provider answered no suggestions".into()),
            }
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        at: CursorPosition,
        chosen: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        let applied = cursor(at)
            .and_then(|at| {
                let request = (lines.to_vec(), at, item(chosen), prefix.to_owned());
                self.synchronous(Request::Apply(request))
            })
            .and_then(|answer| match answer {
                Answer::Applied(applied) => completed(applied),
                _ => Err("the provider answered no completion".to_owned()),
            });
        applied.unwrap_or_else(|error| {
            self.keep(error);
            CompletionResult {
                lines: lines.to_vec(),
                cursor_line: at.line,
                cursor_col: at.col,
            }
        })
    }

    fn should_trigger_file_completion(&self, lines: &[String], at: CursorPosition) -> Option<bool> {
        let decision = cursor(at)
            .and_then(|at| self.synchronous(Request::Trigger((lines.to_vec(), at))))
            .and_then(|answer| match answer {
                Answer::Trigger(decision) => Ok(decision),
                _ => Err("the provider answered no decision".to_owned()),
            });
        decision.unwrap_or_else(|error| {
            self.keep(error);
            None
        })
    }
}
