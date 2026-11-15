//! Ordered protocol registration retaining checked invocation callbacks.
use super::diagnostics::DiagnosticErrorInfo;
use super::types::{
    Api, Invoke, ProviderCall, ProviderExecution, ProviderStreamOptions, SimpleStreamOptions,
    StreamFunction, StreamOptionsFamily,
};
use crate::SharedHandle;
use indexmap::IndexMap;
use std::collections::HashMap;

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
/// Live protocol registrations plus the predecessors their replacements displaced.
#[derive(Default)]
struct ApiRegistrations {
    /// Installed protocol callbacks beneath live registrations, in base-layer order.
    built_ins: IndexMap<String, Registration>,
    /// Checked providers keyed by protocol identity in registration order.
    entries: IndexMap<String, Registration>,
    /// Per protocol, the registrations each newer same-protocol registration replaced,
    /// oldest first, so the last entry is the one removed by the live registration.
    shadowed: HashMap<String, Vec<Registration>>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Process-wide ordered API registrations protected for native callers.
static REGISTRY: std::sync::LazyLock<std::sync::Mutex<ApiRegistrations>> =
    std::sync::LazyLock::new(std::sync::Mutex::default);
#[cfg(target_arch = "wasm32")]
thread_local! {
/// Browser-thread API registrations in insertion order.
static REGISTRY: std::cell::RefCell<ApiRegistrations> = std::cell::RefCell::default(); }
crate::records::platform_store_accessor!(
    /// Access the native shared or browser-local registration map.
    with_registry, REGISTRY, ApiRegistrations
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
/// Insert checked callbacks, keeping an existing protocol's original position.
/// A replaced registration stays recorded, so the replacing registrar's removal
/// restores it instead of leaving the displaced owner registered with nothing.
pub fn register_api_provider(provider: ApiProvider, source_id: Option<String>) {
    let entry = registration(provider, source_id);
    let source = entry.source_id.clone();
    let api = entry.provider.api.clone();
    let retired = with_registry(|registry| {
        match registry.entries.insert(api, entry) {
            // A source replacing its own registration already owns the protocol slot.
            Some(previous) if previous.source_id != source => {
                let record = previous.provider.api.clone();
                registry.shadowed.entry(record).or_default().push(previous);
                None
            }
            previous => previous,
        }
    });
    drop(retired);
}
/// Retain a checked provider handle independently of future registry changes.
#[must_use]
pub fn get_api_provider(api: &str) -> Option<ApiProvider> {
    with_registry(|registry| {
        registry
            .entries
            .get(api)
            .or_else(|| registry.built_ins.get(api))
            .map(|entry| entry.provider.clone())
    })
}
/// Retain the original simple callback for explicit protocol selection.
pub(crate) fn get_api_stream_simple(api: &str) -> Option<ApiStreamSimpleFunction> {
    with_registry(|registry| {
        registry
            .entries
            .get(api)
            .or_else(|| registry.built_ins.get(api))
            .map(|entry| entry.simple.clone())
    })
}
/// Retain checked providers in original registration order.
#[must_use]
pub fn get_api_providers() -> Vec<ApiProvider> {
    with_registry(|registry| {
        let base = registry
            .built_ins
            .iter()
            .map(|(api, entry)| registry.entries.get(api).unwrap_or(entry));
        let others = registry
            .entries
            .iter()
            .filter(|(api, _)| !registry.built_ins.contains_key(api.as_str()))
            .map(|(_, entry)| entry);
        base.chain(others)
            .map(|entry| entry.provider.clone())
            .collect()
    })
}
/// Remove registrations whose current owner matches the supplied identity, drop the
/// shadow copies that identity also superseded, and restore the registration the
/// removed entry displaced.
pub fn unregister_api_providers(source_id: &str) {
    let retired = with_registry(|registry| {
        let mut superseded: Vec<Registration> = Vec::new();
        for list in registry.shadowed.values_mut() {
            superseded
                .extend(list.extract_if(.., |entry| entry.source_id.as_deref() == Some(source_id)));
        }
        let matched: Vec<_> = registry
            .entries
            .iter()
            .filter(|(_, entry)| entry.source_id.as_deref() == Some(source_id))
            .map(|(api, _)| api.clone())
            .collect();
        for api in &matched {
            restore_or_remove(registry, api, &mut superseded);
        }
        superseded
    });
    drop(retired);
}

/// Retire the registration `api` currently holds for the removed identity, installing
/// the registration its replacement displaced, and collect everything retired. The
/// stale copies of that identity are already gone, so the top shadow entry is the
/// registration the removed entry displaced.
fn restore_or_remove(
    registry: &mut ApiRegistrations,
    api: &str,
    superseded: &mut Vec<Registration>,
) {
    let replacement = registry.shadowed.get_mut(api).and_then(Vec::pop);
    let Some(index) = registry.entries.get_index_of(api) else {
        return;
    };
    if let Some(restored) = replacement {
        if let Some((_, slot)) = registry.entries.get_index_mut(index) {
            superseded.push(std::mem::replace(slot, restored));
        }
    } else if let Some((_, entry)) = registry.entries.shift_remove_entry(api) {
        superseded.push(entry);
    }
}
/// Places checked protocol callbacks beneath live registrations, in the supplied order.
///
/// Built-ins hold the first positions. Existing registrations shadow their matching
/// built-ins until removal. Each call replaces the base layer without changing live
/// registrations or their predecessors.
pub fn install_built_in_api_providers(providers: impl IntoIterator<Item = ApiProvider>) {
    let built_ins = providers
        .into_iter()
        .map(|provider| (provider.api.clone(), registration(provider, None)))
        .collect();
    let retired = with_registry(|registry| std::mem::replace(&mut registry.built_ins, built_ins));
    drop(retired);
}

/// Remove all registrations and their base layer without invalidating retained handles.
pub fn clear_api_providers() {
    let retired = with_registry(std::mem::take);
    drop(retired);
}
