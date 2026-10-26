//! The component host's completion providers: the shared fake, with the factory a stacked
//! provider runs handed out through its route.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::sync::atomic::Ordering;

use wasmtime::component::{Accessor, Resource};

use super::host::{Flag, Identity, Route, State};
use super::provider_host::{self as hosted, Lent};
use super::ui_host::{Nested, Ui};
use crate::bindings::host_side::maestro::extension::host;

impl State {
    /// Accepts the host's own provider; the test host never calls an authored provider it
    /// lent.
    fn own(&self, provider: &Resource<Lent>) -> wasmtime::Result<Result<(), String>> {
        Ok(match self.table.get(provider)? {
            Lent::Base => Ok(()),
            Lent::Authored(id) => Err(format!("the test host lent provider {id}")),
        })
    }

    /// Records a provider factory request and hands the factory to the guest with the provider
    /// on top of the stack.
    pub(super) fn add_provider(
        &mut self,
        this: &Resource<Ui>,
        factory: &Resource<Identity>,
    ) -> wasmtime::Result<Resource<Route>> {
        let key = self.table.get(this)?.0;
        let current = match hosted::add(&mut self.observed, key) {
            Ok(current) => current,
            Err(error) => return self.routed(Err(error)),
        };
        let call = host::LocalAutocompleteFactory {
            target: self.table.get(factory)?.0,
            current: self.push(current)?,
            emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
        };
        let mut steps = std::collections::VecDeque::new();
        steps.push_back((host::LocalCall::AutocompleteFactory(call), Nested::Provider));
        self.push(Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }

    /// Keeps the provider a stacked factory returned.
    pub(super) fn stack_provider(&mut self, provider: &Resource<Identity>) {
        let id = provider.rep();
        self.observed.register(format!("provider {id}"), id);
        hosted::push(&mut self.observed, id);
    }
}

impl host::HostCompletionProvider for State {
    fn apply_completion(
        &mut self,
        this: Resource<Lent>,
        lines: Vec<String>,
        cursor: (u32, u32),
        item: host::AutocompleteItem,
        prefix: String,
    ) -> wasmtime::Result<Resource<Route>> {
        if let Err(error) = self.own(&this)? {
            return self.routed(Err(error));
        }
        let operands = (item.value.as_str(), prefix.as_str());
        let (lines, cursor_line, cursor_col) =
            hosted::apply(&mut self.observed, &lines, cursor, operands);
        let completion = host::CompletionResult {
            lines,
            cursor_line,
            cursor_col,
        };
        self.push(Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Completion(completion))),
        })
    }
    fn should_trigger_file_completion(
        &mut self,
        this: Resource<Lent>,
        lines: Vec<String>,
        cursor: (u32, u32),
    ) -> wasmtime::Result<Resource<Route>> {
        if let Err(error) = self.own(&this)? {
            return self.routed(Err(error));
        }
        let decision = hosted::trigger(&mut self.observed, &lines, cursor);
        self.push(Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Trigger(decision))),
        })
    }
    fn drop(&mut self, this: Resource<Lent>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostCompletionProviderWithStore<State> for super::host::Imports {
    fn get_suggestions(
        accessor: &Accessor<State, Self>,
        this: Resource<Lent>,
        lines: Vec<String>,
        cursor: (u32, u32),
        signal: Resource<Flag>,
        force: Option<bool>,
    ) -> impl std::future::Future<
        Output = wasmtime::Result<Result<Option<host::AutocompleteSuggestions>, String>>,
    > + Send {
        std::future::ready(accessor.with(|mut access| {
            let state = access.get();
            if let Err(error) = state.own(&this)? {
                return Ok(Err(error));
            }
            let aborted = state.table.get(&signal)?.0.load(Ordering::SeqCst);
            let found = hosted::suggestions(&mut state.observed, &lines, cursor, (force, aborted));
            Ok(Ok(found.map(|(items, prefix)| {
                host::AutocompleteSuggestions {
                    items: items
                        .into_iter()
                        .map(|(value, label, description)| host::AutocompleteItem {
                            value,
                            label,
                            description,
                        })
                        .collect(),
                    prefix,
                }
            })))
        }))
    }
}
