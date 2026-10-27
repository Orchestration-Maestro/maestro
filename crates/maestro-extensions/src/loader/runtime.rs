//! The shared extension runtime: unbound stubs, the provider queue and the bound route.
use super::provider_order::DeferredProviderOperation;
use crate::types::{read, write};
use crate::{
    EventError, ExtensionExecution,
    types::{
        ExtensionActions, ExtensionRuntime, ExtensionRuntimeState, PendingProviderRegistration,
        ProviderActions, ProviderConfig,
    },
};
use indexmap::IndexMap;
use maestro_catalog::ModelRegistry;
use maestro_models::SharedHandle;
#[cfg(target_arch = "wasm32")]
use std::rc::Weak;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Weak;
use std::{collections::BTreeMap, sync::RwLock};

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
    /// Early factory operations waiting for their ordinary extension position.
    pub(crate) deferred: IndexMap<String, Vec<DeferredProviderOperation>>,
}

/// Bound provider operations with the runner's catalog for absent ones.
#[derive(Clone)]
pub(crate) struct BoundProviders<E: ExtensionExecution> {
    /// Operations supplied by the binder.
    pub(crate) actions: ProviderActions<E>,
    /// The runner owns the catalog; its credentials may retain component callbacks.
    pub(crate) catalog: Weak<ModelRegistry>,
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
            None => Ok(self.catalog()?.register_provider(&name, config)?),
        }
    }
    /// Unregisters through the bound operation, or through the catalog when absent.
    pub(crate) fn unregister_provider(&self, name: &str) -> Result<(), EventError> {
        match &self.actions.unregister_provider {
            Some(unregister) => unregister(name.to_owned()),
            None => Ok(self.catalog()?.unregister_provider(name)?),
        }
    }
    /// Upgrades only for the current call, without retaining the session through its callbacks.
    fn catalog(&self) -> Result<SharedHandle<ModelRegistry>, EventError> {
        self.catalog.upgrade().ok_or_else(|| NOT_INITIALIZED.into())
    }
}

/// Creates a runtime whose actions fail until a runner binds it, except for inert
/// tool refresh, and whose provider registrations queue until then.
#[must_use]
pub fn create_extension_runtime<E: ExtensionExecution>() -> E::Shared<ExtensionRuntime<E>> {
    E::share(ExtensionRuntime {
        host_operations: RwLock::new(None),
        construction: RwLock::new(None),
        #[cfg(not(target_arch = "wasm32"))]
        tool_construction: RwLock::new(None),
        load_time: RwLock::new(None),
        state: ExtensionRuntimeState::default(),
        actions: RwLock::new(E::unbound_actions()),
        pending_provider_registrations: RwLock::new(BTreeMap::new()),
        providers: RwLock::new(ProviderRoute {
            next: 0,
            bound: None,
            deferred: IndexMap::new(),
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
        let pending = PendingProviderRegistration {
            name,
            config,
            extension_path: extension_path.unwrap_or_else(|| UNKNOWN_EXTENSION_PATH.to_owned()),
        };
        if let Some(operations) = route.deferred.get_mut(&pending.extension_path) {
            operations.push(DeferredProviderOperation::Register(pending));
            return Ok(());
        }
        let id = route.next;
        route.next += 1;
        let retired = write(&self.pending_provider_registrations).insert(id, pending);
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
        self.unregister_provider_from(name, None)
    }
    /// Retains early removals with their factory until its ordinary position is reached.
    pub(crate) fn unregister_provider_from(
        &self,
        name: &str,
        path: Option<&str>,
    ) -> Result<(), EventError> {
        let mut route = write(&self.providers);
        if let Some(bound) = &route.bound {
            let bound = bound.clone();
            drop(route);
            return bound.unregister_provider(name);
        }
        if let Some(operations) = path.and_then(|path| route.deferred.get_mut(path)) {
            operations.push(DeferredProviderOperation::Unregister(name.to_owned()));
            return Ok(());
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
