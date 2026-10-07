//! Ordered protocol registrations shared by the execution realm.
use super::{diagnostics::error, types::*};
use std::sync::{Arc, RwLock};

/// Raw callable adapter, including provider-specific options.
pub type ApiStreamFunction = StreamFunction<ProviderStreamOptions>;
/// Simple callable adapter, including reasoning options.
pub type ApiStreamSimpleFunction = StreamFunction<SimpleStreamOptions>;
/// Two callable entry points registered for an open protocol identifier.
#[derive(Clone)]
pub struct ApiProvider {
    /// Exact protocol identifier.
    pub api: Api,
    /// Raw invocation callback.
    pub stream: ApiStreamFunction,
    /// Simple invocation callback.
    pub stream_simple: ApiStreamSimpleFunction,
}
/// Live registered callbacks shared by lookup and enumeration.
pub type ApiProviderHandle = Arc<RwLock<ApiProvider>>;
struct Entry {
    api: Api,
    provider: ApiProviderHandle,
    source: Option<String>,
}
#[cfg(not(target_arch = "wasm32"))]
fn with_registry<R>(f: impl FnOnce(&mut Vec<Entry>) -> R) -> R {
    static REGISTRY: std::sync::Mutex<Vec<Entry>> = std::sync::Mutex::new(Vec::new());
    f(&mut REGISTRY.lock().unwrap_or_else(|p| p.into_inner()))
}
#[cfg(target_arch = "wasm32")]
fn with_registry<R>(f: impl FnOnce(&mut Vec<Entry>) -> R) -> R {
    thread_local! { static REGISTRY: std::cell::RefCell<Vec<Entry>> = const { std::cell::RefCell::new(Vec::new()) }; }
    REGISTRY.with(|r| f(&mut r.borrow_mut()))
}
/// Register wrapped callbacks; replacement retains the original position.
pub fn register_api_provider(provider: ApiProvider, source_id: Option<String>) {
    let api = provider.api.clone();
    let raw_api = api.clone();
    let simple_api = api.clone();
    let raw = provider.stream;
    let simple = provider.stream_simple;
    let wrapped = ApiProvider {
        api: api.clone(),
        stream: std::sync::Arc::new(move |model, context, options| {
            if model.api != raw_api {
                return Err(error(format!(
                    "Mismatched api: {} expected {}",
                    model.api, raw_api
                )));
            }
            raw(model, context, options)
        }),
        stream_simple: std::sync::Arc::new(move |model, context, options| {
            if model.api != simple_api {
                return Err(error(format!(
                    "Mismatched api: {} expected {}",
                    model.api, simple_api
                )));
            }
            simple(model, context, options)
        }),
    };
    let retired = with_registry(|r| {
        let entry = Entry {
            api,
            provider: Arc::new(RwLock::new(wrapped)),
            source: source_id,
        };
        if let Some(i) = r.iter().position(|e| e.api == entry.api) {
            Some(std::mem::replace(&mut r[i], entry))
        } else {
            r.push(entry);
            None
        }
    });
    drop(retired);
}
/// Retrieve live wrapped callbacks, or none when absent.
/// Mutating the provider changes later dispatch without changing its registration key.
pub fn get_api_provider(api: &str) -> Option<ApiProviderHandle> {
    with_registry(|r| r.iter().find(|e| e.api == api).map(|e| e.provider.clone()))
}
/// Enumerate live registrations in insertion order.
pub fn get_api_providers() -> Vec<ApiProviderHandle> {
    with_registry(|r| r.iter().map(|e| e.provider.clone()).collect())
}
/// Remove registrations matching a supplied source, distinct from absent source.
pub fn unregister_api_providers(source_id: &str) {
    let retired = with_registry(|r| {
        let mut retired = vec![];
        let mut i = 0;
        while i < r.len() {
            if r[i].source.as_deref() == Some(source_id) {
                retired.push(r.remove(i));
            } else {
                i += 1;
            }
        }
        retired
    });
    drop(retired);
}
/// Clear registrations without invalidating already retained callbacks.
pub fn clear_api_providers() {
    let retired = with_registry(std::mem::take);
    drop(retired);
}
