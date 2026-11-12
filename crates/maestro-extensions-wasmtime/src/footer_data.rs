//! Footer metadata lent to a footer factory, read from its original native owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_imports::ApiView,
    bindings::maestro::extension::host::HostFooterData,
    callback_imports::retain,
    context_imports::add_resource,
    guest_calls::{Callback, Handle, SynchronousKind},
    lifecycle::State,
    local_resources::{Lease, Registry, effects::SubscriptionEndpoint, held::Footer},
    read_imports::{endpoint, forwards, release},
};
use maestro_extensions::{ExtensionAPI, Local, footer::ReadonlyFooterDataProvider};
use std::{rc::Rc, sync::Arc};
use wasmtime::component::{LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

impl Registry {
    /// Adds the authored branch change listener to the original metadata and retains its
    /// remover; the listener runs under the original extension's callback frame.
    fn branch_change(
        &self,
        footer: &Rc<dyn ReadonlyFooterDataProvider>,
        listener: Handle,
        api: &ExtensionAPI<Local>,
    ) -> Result<SubscriptionEndpoint, String> {
        let (registry, events) = (self.clone(), api.events.clone());
        let remove = footer.on_branch_change(Rc::new(move || {
            let _ = events.invoke_callback(|emitter| {
                let emitter = registry.emitter(emitter.frame())?;
                let kind = SynchronousKind::BranchChange;
                listener.synchronous(&registry, kind, String::new(), emitter)
            });
        }));
        self.subscription_remover(Rc::from(remove))
    }
}

/// Store-safe ownership of the footer metadata lent to one factory.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Reads the current branch.
    pub(crate) fn get_git_branch(&self) -> Result<Option<String>, String> {
        self.0
            .run::<Footer, _>(|footer, _| Ok(footer.get_git_branch()))
    }

    /// Reads the extension statuses in insertion order.
    pub(crate) fn get_extension_statuses(&self) -> Result<Vec<(String, String)>, String> {
        self.0
            .run::<Footer, _>(|footer, _| Ok(footer.get_extension_statuses().iter().collect()))
    }

    /// Reads the count of providers with available models.
    pub(crate) fn get_available_provider_count(&self) -> Result<f64, String> {
        self.0
            .run::<Footer, _>(|footer, _| Ok(footer.get_available_provider_count()))
    }

    /// Adds an authored branch change listener under the original extension.
    pub(crate) fn on_branch_change(
        &self,
        listener: Handle,
        api: crate::api_reads::Endpoint,
    ) -> Result<SubscriptionEndpoint, String> {
        self.0.run::<Footer, _>(move |footer, registry| {
            registry
                .facade(&api)
                .and_then(|api| registry.branch_change(footer, listener, &api))
        })
    }
}

impl HostFooterData for State {
    fn get_git_branch(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(endpoint(self, &this)?.get_git_branch())
    }

    fn get_extension_statuses(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Vec<(String, String)>, String>> {
        Ok(endpoint(self, &this)?.get_extension_statuses())
    }

    fn get_available_provider_count(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<f64, String>> {
        Ok(endpoint(self, &this)?.get_available_provider_count())
    }

    fn on_branch_change(
        &mut self,
        this: Resource<Endpoint>,
        listener: Resource<Callback>,
    ) -> wasmtime::Result<Result<Resource<SubscriptionEndpoint>, String>> {
        let listener = retain(self, &listener)?;
        let api = self.api().clone();
        match endpoint(self, &this)?.on_branch_change(listener, api) {
            Ok(subscription) => Ok(Ok(self.ctx().table.push(subscription)?)),
            Err(error) => Ok(Err(error)),
        }
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every footer metadata method to its trait implementation.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "footer-data")?;
    forwards! { host, "footer-data", HostFooterData, Endpoint,
        "get-git-branch" => get_git_branch();
        "get-extension-statuses" => get_extension_statuses();
        "get-available-provider-count" => get_available_provider_count();
        "on-branch-change" => on_branch_change(listener: Resource<Callback>);
    }
    Ok(())
}
