//! The shared extension runtime: unbound stubs, the provider queue and the bound route.
use crate::types::{read, write};
use crate::{
    EventError, ExtensionExecution,
    types::{
        ExtensionActions, ExtensionRuntime, ExtensionRuntimeState, PendingProviderRegistration,
        ProviderActions, ProviderConfig,
    },
};
use maestro_catalog::ModelRegistry;
use maestro_models::SharedHandle;
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};

/// Failure of an action called before a runner binds the runtime.
pub(crate) const NOT_INITIALIZED: &str =
    "Extension runtime not initialized. Action methods cannot be called during extension loading.";
/// Failure of a model selection requested before a runner binds the runtime.
pub(crate) const SET_MODEL_NOT_INITIALIZED: &str = "Extension runtime not initialized";
/// Attribution of a queued registration that named no extension.
const UNKNOWN_EXTENSION_PATH: &str = "<unknown>";

/// Queue ordinals and the optional bound route.
pub(crate) struct ProviderRoute<E: ExtensionExecution> {
    /// Next registration ordinal, never reused during a flush.
    pub(crate) next: u64,
    /// Bound operations after the queue completes.
    pub(crate) bound: Option<BoundProviders<E>>,
}

/// Bound provider operations with the runner's catalog for absent ones.
#[derive(Clone)]
pub(crate) struct BoundProviders<E: ExtensionExecution> {
    /// Operations supplied by the binder.
    pub(crate) actions: ProviderActions<E>,
    /// The runner's catalog.
    pub(crate) catalog: SharedHandle<ModelRegistry>,
}

impl<E: ExtensionExecution> BoundProviders<E> {
    /// Registers through the bound operation, or through the catalog when absent.
    ///
    /// The configuration is the platform handle, retained as the browser's local owner or
    /// the native shared one.
    pub(crate) fn register_provider(
        &self,
        name: String,
        config: SharedHandle<RwLock<ProviderConfig>>,
    ) -> Result<(), EventError> {
        match &self.actions.register_provider {
            Some(register) => register(name, config),
            None => Ok(self.catalog.register_provider(&name, config)?),
        }
    }
    /// Unregisters through the bound operation, or through the catalog when absent.
    pub(crate) fn unregister_provider(&self, name: &str) -> Result<(), EventError> {
        match &self.actions.unregister_provider {
            Some(unregister) => unregister(name.to_owned()),
            None => Ok(self.catalog.unregister_provider(name)?),
        }
    }
}

/// Creates a runtime whose actions fail until a runner binds it, except for inert
/// tool refresh, and whose provider registrations queue until then.
#[must_use]
pub fn create_extension_runtime<E: ExtensionExecution>() -> E::Shared<ExtensionRuntime<E>> {
    E::share(ExtensionRuntime {
        state: Arc::new(ExtensionRuntimeState::default()),
        actions: RwLock::new(E::unbound_actions()),
        pending_provider_registrations: RwLock::new(BTreeMap::new()),
        providers: RwLock::new(ProviderRoute {
            next: 0,
            bound: None,
        }),
    })
}

impl<E: ExtensionExecution> ExtensionRuntime<E> {
    /// Returns one currently bound operation.
    pub(crate) fn action<F: ?Sized>(
        &self,
        select: fn(&ExtensionActions<E>) -> &E::Shared<F>,
    ) -> E::Shared<F> {
        select(&read(&self.actions)).clone()
    }
    /// Queues a registration before binding; afterwards registers it immediately.
    ///
    /// `None` attributes a queued registration to `<unknown>`. The configuration is the
    /// platform handle, retained as the browser's local owner or the native shared one.
    ///
    /// # Errors
    /// Returns the bound operation's or the catalog's error after binding.
    pub fn register_provider(
        &self,
        name: String,
        config: SharedHandle<RwLock<ProviderConfig>>,
        extension_path: Option<String>,
    ) -> Result<(), EventError> {
        let mut route = write(&self.providers);
        if let Some(bound) = &route.bound {
            let bound = bound.clone();
            drop(route);
            return bound.register_provider(name, config);
        }
        let id = route.next;
        route.next += 1;
        let retired = write(&self.pending_provider_registrations).insert(
            id,
            PendingProviderRegistration {
                name,
                config,
                extension_path: extension_path.unwrap_or_else(|| UNKNOWN_EXTENSION_PATH.to_owned()),
            },
        );
        drop(route);
        drop(retired);
        Ok(())
    }
    /// Removes every queued registration of `name` before binding; afterwards
    /// unregisters it immediately.
    ///
    /// # Errors
    /// Returns the bound operation's or the catalog's error after binding.
    pub fn unregister_provider(&self, name: &str) -> Result<(), EventError> {
        let route = read(&self.providers);
        if let Some(bound) = &route.bound {
            let bound = bound.clone();
            drop(route);
            return bound.unregister_provider(name);
        }
        let removed: Vec<_> = write(&self.pending_provider_registrations)
            .extract_if(.., |_, pending| pending.name == name)
            .collect();
        drop(route);
        drop(removed);
        Ok(())
    }
    /// Visits the next live registration, or publishes the bound route when none remain.
    pub(crate) fn next_or_bind(
        &self,
        cursor: u64,
        bound: &BoundProviders<E>,
    ) -> Option<(u64, PendingProviderRegistration)> {
        let mut route = write(&self.providers);
        let mut queue = write(&self.pending_provider_registrations);
        if let Some((id, pending)) = queue.range(cursor..).next() {
            return Some((*id, pending.clone()));
        }
        let retired = std::mem::take(&mut *queue);
        let old = route.bound.replace(bound.clone());
        drop(queue);
        drop(route);
        drop(retired);
        drop(old);
        None
    }
}
