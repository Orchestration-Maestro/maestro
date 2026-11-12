//! Lent completion providers and the authored provider factories stacked on them, served on
//! the local owner.
use super::{Admission, Capability, Registry, pump::State};
use crate::guest_calls::providers::{
    Native, ProviderEndpoint, SuggestionRequest, Suggestions, native_cursor,
};
use crate::guest_calls::widget_calls::Item;
use maestro_extensions::presentation::CompletionOptions;

impl Registry {
    /// Lends a native completion provider to the guest.
    pub(crate) fn lend_provider(&self, provider: Native) -> Result<ProviderEndpoint, String> {
        self.insert(Capability::Provider(provider))
            .map(ProviderEndpoint)
    }

    /// Stacks an authored provider factory on the original UI's completion provider.
    pub(crate) fn add_autocomplete_provider(
        &self,
        ui: &maestro_extensions::ExtensionUIContext,
        factory: crate::guest_calls::Handle,
        api: &crate::api_reads::Endpoint,
    ) -> Result<(), String> {
        let factory = factory.provider_factory(self.clone(), &self.facade(api)?);
        ui.add_autocomplete_provider(factory)
            .map_err(|error| error.to_string())
    }

    /// The native completion provider behind a lent provider resource.
    fn provider(&self, id: u64) -> Result<Native, String> {
        match self.capability(id).as_deref() {
            Some(Capability::Provider(provider)) => Ok(provider.clone()),
            _ => Err("extension completion provider resource ended".to_owned()),
        }
    }
}

/// The canonical form of native suggestions.
fn suggestions(
    suggestions: maestro_extensions::presentation::AutocompleteSuggestions,
) -> Suggestions {
    Suggestions {
        items: suggestions
            .items
            .into_iter()
            .map(|item| Item {
                value: item.value,
                label: item.label,
                description: item.description,
            })
            .collect(),
        prefix: suggestions.prefix,
    }
}

impl State {
    /// Starts one suggestion request on a lent provider with the caller's signal.
    pub(super) fn suggestions(&self, id: u64, request: SuggestionRequest, admission: Admission) {
        let registry = &self.registry;
        let operands = registry.provider(id).and_then(|provider| {
            let signal = registry.cancellation(&request.signal)?;
            Ok((provider, signal, native_cursor(request.cursor)?))
        });
        let (provider, signal, at) = match operands {
            Ok(operands) => operands,
            Err(error) => {
                let _ = request.reply.send(Err(error));
                return;
            }
        };
        let (lines, force, reply) = (request.lines, request.force, request.reply);
        self.admit(
            Box::pin(async move {
                let options = CompletionOptions {
                    signal: &signal,
                    force,
                };
                let found = provider.get_suggestions(&lines, at, options).await;
                let found = found.map(|found| found.map(suggestions));
                let _ = reply.send(found.map_err(|error| error.to_string()));
            }),
            admission,
        );
    }
}
