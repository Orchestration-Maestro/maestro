//! Ordered protocol registration retaining checked invocation callbacks.
use super::diagnostics::DiagnosticErrorInfo;
use super::types::{Api, ProviderStreamOptions, SimpleStreamOptions, StreamFunction};
use crate::builtins::built_in_providers;
use indexmap::IndexMap;
use std::sync::Arc;

/// Raw invocation callback retained by registration.
pub type ApiStreamFunction = StreamFunction<ProviderStreamOptions>;
/// Simple invocation callback retained by registration.
pub type ApiStreamSimpleFunction = StreamFunction<SimpleStreamOptions>;
/// Protocol identifier paired with raw and simple invocation callbacks.
#[derive(Clone)]
pub struct ApiProvider {
    /// Open protocol identifier.
    pub api: Api,
    /// Checked raw invocation.
    pub stream: ApiStreamFunction,
    /// Checked simple invocation.
    pub stream_simple: ApiStreamSimpleFunction,
}
/// Registered provider with its optional extension ownership.
struct Registration {
    /// Provider callbacks exposed by this registration.
    provider: ApiProvider,
    /// Identifier of the extension that owns this registration.
    source_id: Option<String>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Process-wide ordered API registrations protected for native callers.
static REGISTRY: std::sync::LazyLock<std::sync::Mutex<IndexMap<String, Registration>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(seeded()));
#[cfg(target_arch = "wasm32")]
thread_local! {
/// Browser-thread API registrations in insertion order.
static REGISTRY: std::cell::RefCell<IndexMap<String, Registration>> = std::cell::RefCell::new(seeded()); }
/// Access the native shared or browser-local registration map.
fn with_registry<T>(operation: impl FnOnce(&mut IndexMap<String, Registration>) -> T) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        operation(
            &mut REGISTRY
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        REGISTRY.with_borrow_mut(operation)
    }
}
/// Wrap a stream callback with an API-identity check.
fn checked<O: 'static>(api: String, callback: StreamFunction<O>) -> StreamFunction<O> {
    Arc::new(move |model, context, options| {
        if model.api != api {
            return Err(DiagnosticErrorInfo {
                name: Some("Error".into()),
                message: format!("Mismatched api: {} expected {api}", model.api),
                stack: None,
                code: None,
            });
        }
        callback(model, context, options)
    })
}
/// Pair checked callbacks with their owner.
fn registration(provider: ApiProvider, source_id: Option<String>) -> Registration {
    let ApiProvider {
        api,
        stream,
        stream_simple,
    } = provider;
    Registration {
        provider: ApiProvider {
            stream: checked(api.clone(), stream),
            stream_simple: checked(api.clone(), stream_simple),
            api,
        },
        source_id,
    }
}
/// The bundled protocols, registered before the first lookup or change.
fn seeded() -> IndexMap<String, Registration> {
    built_in_providers()
        .into_iter()
        .map(|provider| (provider.api.clone(), registration(provider, None)))
        .collect()
}
/// Insert checked callbacks, keeping an existing protocol's original position.
pub fn register_api_provider(provider: ApiProvider, source_id: Option<String>) {
    let entry = registration(provider, source_id);
    let retired = with_registry(|registry| registry.insert(entry.provider.api.clone(), entry));
    drop(retired);
}
/// Retain a checked provider handle independently of future registry changes.
#[must_use]
pub fn get_api_provider(api: &str) -> Option<ApiProvider> {
    with_registry(|registry| registry.get(api).map(|entry| entry.provider.clone()))
}
/// Retain checked providers in original registration order.
#[must_use]
pub fn get_api_providers() -> Vec<ApiProvider> {
    with_registry(|registry| {
        registry
            .values()
            .map(|entry| entry.provider.clone())
            .collect()
    })
}
/// Remove registrations whose current owner matches the supplied identity.
pub fn unregister_api_providers(source_id: &str) {
    let retired = with_registry(|registry| {
        let keys: Vec<_> = registry
            .iter()
            .filter(|(_, entry)| entry.source_id.as_deref() == Some(source_id))
            .map(|(api, _)| api.clone())
            .collect();
        keys.into_iter()
            .filter_map(|key| registry.shift_remove(&key))
            .collect::<Vec<_>>()
    });
    drop(retired);
}
/// Remove all registrations without invalidating retained handles.
pub fn clear_api_providers() {
    let retired = with_registry(std::mem::take);
    drop(retired);
}
