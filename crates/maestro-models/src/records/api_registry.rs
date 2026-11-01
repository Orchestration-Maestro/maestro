//! Ordered protocol registration retaining checked invocation callbacks.
use super::diagnostics::DiagnosticErrorInfo;
use super::types::{
    Api, Invoke, ProviderCall, ProviderExecution, ProviderStreamOptions, SimpleStreamOptions,
    StreamFunction, StreamOptionsFamily,
};
use crate::SharedHandle;
use crate::builtins::built_in_providers;
use indexmap::IndexMap;

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
    /// Original simple callback shared with the checked wrapper.
    simple: ApiStreamSimpleFunction,
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
crate::records::platform_store_accessor!(
    /// Access the native shared or browser-local registration map.
    with_registry, REGISTRY, IndexMap<String, Registration>
);
/// API guard retaining the original callback independently of registry changes.
struct Checked<K: StreamOptionsFamily> {
    /// Registered protocol identity.
    api: String,
    /// Caller-supplied callback owner.
    callback: StreamFunction<K>,
}
impl<E: ProviderExecution, K: StreamOptionsFamily> Invoke<E, ProviderCall<E, K::Options<E>>>
    for Checked<K>
{
    type Return = Result<super::types::AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, input: ProviderCall<E, K::Options<E>>) -> Self::Return {
        if input.model.api != self.api {
            return Err(DiagnosticErrorInfo::error(format!(
                "Mismatched api: {} expected {}",
                input.model.api, self.api
            )));
        }
        self.callback.call(
            input.model,
            input.context,
            E::submission(input.options, input.submit),
        )
    }
}
/// Wrap a stream callback with the shared API-identity check.
fn checked<K: StreamOptionsFamily>(api: String, callback: StreamFunction<K>) -> StreamFunction<K> {
    SharedHandle::new(Checked { api, callback })
}
/// Pair checked callbacks with their owner.
fn registration(provider: ApiProvider, source_id: Option<String>) -> Registration {
    let ApiProvider {
        api,
        stream,
        stream_simple,
    } = provider;
    Registration {
        simple: stream_simple.clone(),
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
/// Retain the original simple callback for explicit protocol selection.
pub(crate) fn get_api_stream_simple(api: &str) -> Option<ApiStreamSimpleFunction> {
    with_registry(|registry| registry.get(api).map(|entry| entry.simple.clone()))
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
