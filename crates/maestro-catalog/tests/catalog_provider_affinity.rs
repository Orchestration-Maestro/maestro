#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Public catalog callbacks retain the original execution-family carriers.
#[path = "../../maestro-models/tests/support/original_options.rs"]
mod original_options;
use crate::{dynamic, support};
use maestro_catalog::{ModelRegistry, ProviderConfigInput};
use maestro_models::*;
use std::sync::{
    Arc, RwLock, Weak,
    atomic::{AtomicUsize, Ordering},
};

/// Callback ownership includes an observable, reentrant final release.
struct CatalogProvider {
    /// Same complete carrier witness used through the direct registry writer.
    options: original_options::OptionsProvider,
    /// Non-owning path back to the configuration containing this callback.
    config: Weak<RwLock<ProviderConfigInput>>,
    /// Written after confirming both configuration and API registry are unlocked.
    released: Arc<AtomicUsize>,
}
impl<E: original_options::Produce> Invoke<E, ProviderCall<E, StreamOptionsInput<E>>>
    for CatalogProvider
{
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, StreamOptionsInput<E>>) -> Self::Return {
        self.options.invoke(call)
    }
}
impl Drop for CatalogProvider {
    fn drop(&mut self) {
        if let Some(config) = self.config.upgrade() {
            drop(
                config
                    .try_write()
                    .expect("callback must retire outside configuration guard"),
            );
        }
        let _ = get_api_provider("catalog-affinity");
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}
/// Construct the actual public input, with no provider work during configuration.
fn config(
    released: &Arc<AtomicUsize>,
) -> (
    SharedHandle<RwLock<ProviderConfigInput>>,
    Arc<CatalogProvider>,
) {
    let config = dynamic::config(&["m"]);
    config.write().unwrap().api = Some("catalog-affinity".into());
    let owner = Arc::new(CatalogProvider {
        options: original_options::OptionsProvider::default(),
        config: SharedHandle::downgrade(&config),
        released: released.clone(),
    });
    config.write().unwrap().callbacks.stream_simple = Some(owner.clone());
    (config, owner)
}
/// Run and join every admitted producer before releasing callback ownership.
fn exercise(provider: &ApiProvider, model: &Model, owner: &CatalogProvider) {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(
            tokio::task::LocalSet::new().run_until(original_options::exercise(
                provider,
                model,
                &owner.options,
            )),
        );
}
/// Resolve the actual catalog descriptor without substituting a fixture model.
fn model(registry: &ModelRegistry) -> Model {
    registry.find("p", "m").unwrap().read().unwrap().clone()
}
#[test]
fn catalog_registration_keeps_original_option_families() {
    if dynamic::isolated("catalog_registration_keeps_original_option_families") {
        return;
    }
    let released = Arc::new(AtomicUsize::new(0));
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let (config, owner) = config(&released);
    registry.register_provider("p", config.clone()).unwrap();
    let _ = registry.get_all();
    let model = model(&registry);
    assert!(owner.options.observed.lock().unwrap().is_empty());
    let retained = get_api_provider("catalog-affinity").unwrap();
    exercise(&retained, &model, &owner);
    registry.unregister_provider("p").unwrap();
    config.write().unwrap().callbacks.stream_simple = None;
    drop(owner);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(retained);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}
#[test]
fn catalog_replacement_keeps_callback_and_source_ownership() {
    if dynamic::isolated("catalog_replacement_keeps_callback_and_source_ownership") {
        return;
    }
    let released = Arc::new(AtomicUsize::new(0));
    let registry = ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap();
    let (first_config, first) = config(&released);
    registry
        .register_provider("p", first_config.clone())
        .unwrap();
    let old = get_api_provider("catalog-affinity").unwrap();
    let (replacement_config, replacement) = config(&released);
    registry
        .register_provider("p", replacement_config.clone())
        .unwrap();
    let current = get_api_provider("catalog-affinity").unwrap();
    assert!(!Arc::ptr_eq(&old.stream, &current.stream));
    assert!(first.options.observed.lock().unwrap().is_empty());
    assert!(replacement.options.observed.lock().unwrap().is_empty());
    exercise(&old, &model(&registry), &first);
    exercise(&current, &model(&registry), &replacement);
    registry.refresh().unwrap();
    let replayed = get_api_provider("catalog-affinity").unwrap();
    exercise(&replayed, &model(&registry), &replacement);
    let first_weak = Arc::downgrade(&first);
    let replacement_weak = Arc::downgrade(&replacement);
    let old_model = model(&registry);
    registry.unregister_provider("p").unwrap();
    assert!(get_api_provider("catalog-affinity").is_none());
    // Earlier retained providers work even after the public source has disappeared.
    exercise(&old, &old_model, &first);
    exercise(&replayed, &old_model, &replacement);
    first_config.write().unwrap().callbacks.stream_simple = None;
    replacement_config.write().unwrap().callbacks.stream_simple = None;
    drop(first);
    drop(replacement);
    drop(old);
    assert!(first_weak.upgrade().is_none());
    assert_eq!(released.load(Ordering::SeqCst), 1);
    drop(current);
    let raw_alias = replayed.stream;
    let simple_alias = replayed.stream_simple;
    drop(raw_alias);
    assert!(replacement_weak.upgrade().is_some());
    drop(simple_alias);
    assert!(replacement_weak.upgrade().is_none());
    assert_eq!(released.load(Ordering::SeqCst), 2);
}
