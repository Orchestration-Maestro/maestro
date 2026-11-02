#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! Public callback re-entry and phase-specific catalog observations.
use crate::{dynamic, request_support, support};
use maestro_catalog::ModelRegistry;
use maestro_models::SharedHandle;
use std::sync::{Arc, Mutex};

#[test]
fn registration_transform_can_reenter_catalog() {
    if dynamic::isolated("registration_transform_can_reenter_catalog") {
        return;
    }
    for throws in [false, true] {
        let registry =
            Arc::new(ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap());
        registry.get_all().write().unwrap().clear();
        registry
            .auth_storage()
            .set("outer", request_support::tokens(f64::MAX));
        let weak = Arc::downgrade(&registry);
        let observed = Arc::new(Mutex::new(None));
        let saved = observed.clone();
        let config = dynamic::config(&["outer-model"]);
        config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
            calls: None,
            id: "ignored",
            name: "Outer",
            transform: Arc::new(move |models| {
                let registry = weak.upgrade().unwrap();
                *saved.lock().unwrap() = registry.find("outer", "outer-model");
                assert!(saved.lock().unwrap().is_some());
                registry
                    .register_provider("nested", dynamic::config(&["nested-model"]))
                    .unwrap();
                assert!(registry.find("nested", "nested-model").is_some());
                if throws {
                    Err(dynamic::account_failure("transform refused"))
                } else {
                    Ok(models)
                }
            }),
        }));
        let result = registry.register_provider("outer", config);
        assert!(Arc::ptr_eq(
            observed.lock().unwrap().as_ref().unwrap(),
            &registry.find("outer", "outer-model").unwrap()
        ));
        if throws {
            dynamic::assert_account_failure(&result.unwrap_err(), "transform refused");
            assert!(registry.find("nested", "nested-model").is_some());
        } else {
            result.unwrap();
            assert!(registry.find("nested", "nested-model").is_none());
        }
        registry.unregister_provider("nested").unwrap();
        if throws {
            assert!(registry.find("outer", "outer-model").is_none());
        } else {
            assert!(registry.find("outer", "outer-model").is_some());
        }
        maestro_models::reset_oauth_providers();
    }
}

/// Empty published view for independently recorded callback scenarios.
fn registry() -> Arc<ModelRegistry> {
    let registry =
        Arc::new(ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap());
    registry.get_all().write().unwrap().clear();
    registry
}
/// Attach a supplied transformation using the existing callback owner.
fn transforming(
    ids: &[&str],
    effect: impl for<'m> Fn(
        Vec<std::borrow::Cow<'m, maestro_models::Model>>,
    ) -> Result<
        Vec<std::borrow::Cow<'m, maestro_models::Model>>,
        maestro_models::OAuthError,
    > + Send
    + Sync
    + 'static,
) -> Arc<std::sync::RwLock<maestro_catalog::ProviderConfigInput>> {
    let config = dynamic::config(ids);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(dynamic::OAuth {
        calls: None,
        id: "ignored",
        name: "Outer",
        transform: Arc::new(effect),
    }));
    config
}
/// Recorded input and expected observable selection.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionCase {
    input: SelectionInput,
    expected: SelectionExpected,
}
/// The reference callback edit.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionInput {
    edit: String,
}
/// Selected descriptors and retained registration order.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionExpected {
    models: Vec<Selected>,
    retained: Vec<String>,
    error: Option<String>,
}
/// Public descriptor fields observed by the recorder.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Selected {
    id: String,
    name: String,
}

#[test]
fn selected_transform_return_wins_over_reentrant_writes() {
    if dynamic::isolated("selected_transform_return_wins_over_reentrant_writes") {
        return;
    }
    let cases: Vec<SelectionCase> =
        serde_json::from_str(include_str!("reentry/selection.json")).unwrap();
    for case in cases {
        let registry = registry();
        registry
            .auth_storage()
            .set("outer", request_support::tokens(f64::MAX));
        let weak = Arc::downgrade(&registry);
        let nested_handle = Arc::new(Mutex::new(std::sync::Weak::new()));
        let saved = nested_handle.clone();
        let edit = case.input.edit;
        let config = transforming(&["one", "two"], move |mut models| {
            let registry = weak.upgrade().unwrap();
            let nested = dynamic::config(&["nested-model"]);
            nested.write().unwrap().models.as_mut().unwrap()[0].name = "nested-model".into();
            *saved.lock().unwrap() = Arc::downgrade(&nested);
            registry.register_provider("nested", nested).unwrap();
            edit_selection(&registry, &edit, &mut models)?;
            Ok(models)
        });
        for model in config.write().unwrap().models.as_mut().unwrap() {
            model.name.clone_from(&model.id);
        }
        let retained = SharedHandle::downgrade(&config);
        let error = registry
            .register_provider("outer", config)
            .err()
            .map(|e| e.to_string());
        assert_eq!(error, case.expected.error);
        let actual: Vec<_> = registry
            .get_all()
            .read()
            .unwrap()
            .iter()
            .map(|m| {
                let m = m.read().unwrap();
                Selected {
                    id: m.id.clone(),
                    name: m.name.clone(),
                }
            })
            .collect();
        assert_eq!(actual, case.expected.models);
        assert_retained_order(
            &registry,
            &[
                ("nested", nested_handle.lock().unwrap().clone()),
                ("outer", retained),
            ],
            &case.expected.retained,
        );
        maestro_models::reset_oauth_providers();
    }
}

#[test]
fn refresh_visits_live_registrations_in_order() {
    if dynamic::isolated("refresh_visits_live_registrations_in_order") {
        return;
    }
    replay_case("push", &["outer", "outer", "pushed", "pushed"]);
    replay_case("replace", &["outer", "replacement", "replacement"]);
    replay_case("readd", &["outer", "outer", "readded", "readded"]);
    replay_case("remove", &["outer", "later", "later"]);
    replay_work();
}
/// Replay one synchronous edit and observe a subsequent invocation separately.
fn replay_case(edit: &'static str, expected: &[&str]) {
    let registry = registry();
    let active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let events = Arc::new(Mutex::new(Vec::new()));
    let weak = Arc::downgrade(&registry);
    let enabled = active.clone();
    let seen = events.clone();
    let outer = transforming(&["outer-model"], move |models| {
        seen.lock().unwrap().push("outer");
        if enabled.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let registry = weak.upgrade().unwrap();
            edit_registrations(&registry, edit, &seen);
        }
        Ok(models)
    });
    for provider in ["outer", "later", "pushed"] {
        registry
            .auth_storage()
            .set(provider, request_support::tokens(f64::MAX));
    }
    registry.register_provider("outer", outer).unwrap();
    registry
        .register_provider("later", recording(&events, "later"))
        .unwrap();
    events.lock().unwrap().clear();
    active.store(true, std::sync::atomic::Ordering::SeqCst);
    registry.refresh().unwrap();
    assert_eq!(*events.lock().unwrap(), expected);
    events.lock().unwrap().clear();
    registry.refresh().unwrap();
    assert_eq!(
        *events.lock().unwrap(),
        if edit == "remove" {
            vec!["later"]
        } else if edit == "push" {
            vec!["outer", "pushed"]
        } else if edit == "replace" {
            vec!["outer", "replacement"]
        } else {
            vec!["outer", "readded"]
        }
    );
    maestro_models::reset_oauth_providers();
}
/// Apply callback edits through public registration operations.
fn edit_registrations(registry: &ModelRegistry, edit: &str, seen: &Arc<Mutex<Vec<&'static str>>>) {
    match edit {
        "push" | "readd" => {
            let (provider, event) = if edit == "push" {
                ("pushed", "pushed")
            } else {
                ("later", "readded")
            };
            registry.unregister_provider("later").unwrap();
            registry
                .register_provider(provider, recording(seen, event))
                .unwrap();
        }
        "replace" => registry
            .register_provider("later", recording(seen, "replacement"))
            .unwrap(),
        "remove" => registry.unregister_provider("outer").unwrap(),
        _ => unreachable!(),
    }
}
/// Separate visit counts from logarithmic successor lookup work.
fn replay_work() {
    for size in [32, 64] {
        let registry = registry();
        let events = Arc::new(Mutex::new(Vec::new()));
        for index in 0..size {
            let name = format!("provider-{index}");
            registry
                .auth_storage()
                .set(&name, request_support::tokens(f64::MAX));
            registry
                .register_provider(&name, recording(&events, "visit"))
                .unwrap();
        }
        events.lock().unwrap().clear();
        registry.refresh().unwrap();
        assert_eq!(events.lock().unwrap().len(), size);
        println!(
            "REPLAY-WORK providers={size} visits={size}; one logarithmic successor lookup per visit"
        );
        maestro_models::reset_oauth_providers();
    }
}
/// Record the actual callback sequence without synthesizing events from presence.
fn recording(
    events: &Arc<Mutex<Vec<&'static str>>>,
    name: &'static str,
) -> Arc<std::sync::RwLock<maestro_catalog::ProviderConfigInput>> {
    let events = events.clone();
    transforming(&[name], move |models| {
        events.lock().unwrap().push(name);
        Ok(models)
    })
}

#[test]
fn refresh_stops_at_first_reentrant_transform_failure() {
    if dynamic::isolated("refresh_stops_at_first_reentrant_transform_failure") {
        return;
    }
    let registry = registry();
    let active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let enabled = active.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = events.clone();
    let weak = Arc::downgrade(&registry);
    let outer = transforming(&["outer-model"], move |models| {
        seen.lock().unwrap().push("outer");
        assert!(
            weak.upgrade()
                .unwrap()
                .find("outer", "outer-model")
                .is_some()
        );
        if enabled.load(std::sync::atomic::Ordering::SeqCst) {
            Err(dynamic::account_failure("refresh refused"))
        } else {
            Ok(models)
        }
    });
    registry
        .auth_storage()
        .set("outer", request_support::tokens(f64::MAX));
    registry
        .auth_storage()
        .set("later", request_support::tokens(f64::MAX));
    let later = recording(&events, "later");
    let retained = SharedHandle::downgrade(&later);
    registry.register_provider("outer", outer).unwrap();
    registry.register_provider("later", later).unwrap();
    events.lock().unwrap().clear();
    active.store(true, std::sync::atomic::Ordering::SeqCst);
    dynamic::assert_account_failure(&registry.refresh().unwrap_err(), "refresh refused");
    assert_eq!(*events.lock().unwrap(), ["outer"]);
    assert!(registry.find("outer", "outer-model").is_some());
    assert!(registry.find("later", "later").is_none());
    assert!(retained.upgrade().is_some());
    dynamic::assert_account_failure(
        &registry.unregister_provider("later").unwrap_err(),
        "refresh refused",
    );
    assert!(retained.upgrade().is_none());
    active.store(false, std::sync::atomic::Ordering::SeqCst);
    events.lock().unwrap().clear();
    registry.refresh().unwrap();
    assert_eq!(*events.lock().unwrap(), ["outer"]);
}

#[test]
fn request_auth_keeps_early_provider_and_late_model_sources() {
    if dynamic::isolated("request_auth_keeps_early_provider_and_late_model_sources") {
        return;
    }
    for (early, late) in [("old", "new"), ("new", "old")] {
        let registry = registry();
        let config = header_config(early, early);
        registry.register_provider("p", config).unwrap();
        let model = registry.find("p", "m").unwrap();
        let weak = Arc::downgrade(&registry);
        let live = model.clone();
        let mut oauth = request_support::OAuth::new("p", "Provider");
        oauth.refresh = Some(Box::new(move |mut credentials| {
            let registry = weak.upgrade().unwrap();
            registry
                .register_provider("p", header_config(late, late))
                .unwrap();
            registry
                .register_provider("renamed", header_config(early, late))
                .unwrap();
            let live = live.clone();
            Box::pin(async move {
                std::future::ready(()).await;
                live.write().unwrap().provider = "renamed".into();
                credentials.expires = f64::MAX;
                Ok(credentials)
            })
        }));
        let _registration = oauth.register();
        registry
            .auth_storage()
            .set("p", request_support::tokens(0.0));
        request_support::success(
            request_support::run(registry.get_api_key_and_headers(&model)),
            Some("unused-access"),
            &[("P", early), ("M", late)],
        );
        assert_eq!(model.read().unwrap().provider, "renamed");
    }
}
/// Unequal independently selected provider and model request sources.
fn header_config(
    provider: &str,
    model: &str,
) -> Arc<std::sync::RwLock<maestro_catalog::ProviderConfigInput>> {
    let config = dynamic::config(&["m"]);
    {
        let mut config = config.write().unwrap();
        config.headers = Some([("P".into(), provider.into())].into());
        config.models.as_mut().unwrap()[0].headers = Some([("M".into(), model.into())].into());
    }
    config
}

/// Supplied configuration effects with an independently controlled command result.
struct EffectOperations {
    /// Caller code the configured command runs.
    effect: Box<dyn Fn(&str) + Send + Sync>,
    /// Completed command output.
    output: Option<&'static str>,
    /// Commands asked for, in order.
    calls: Arc<Mutex<Vec<String>>>,
}
impl maestro_credentials::ConfigValueOperations for EffectOperations {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        self.calls.lock().unwrap().push(command.to_owned());
        (self.effect)(command);
        self.output.map(|s| s.as_bytes().to_vec())
    }
}
/// Operations that reach their own catalog once ownership is established.
fn reentrant(
    slot: &Arc<std::sync::OnceLock<std::sync::Weak<ModelRegistry>>>,
    effect: impl Fn(&ModelRegistry, &str) + Send + Sync + 'static,
    output: Option<&'static str>,
) -> Arc<EffectOperations> {
    let slot = Arc::clone(slot);
    Arc::new(EffectOperations {
        effect: Box::new(move |command| {
            let registry = slot.get().unwrap().upgrade().unwrap();
            effect(&registry, command);
        }),
        output,
        calls: Arc::default(),
    })
}
/// Offline catalog over `values`, bound to the handle that reaches it.
fn bind(
    slot: &Arc<std::sync::OnceLock<std::sync::Weak<ModelRegistry>>>,
    values: &Arc<EffectOperations>,
) -> Arc<ModelRegistry> {
    let registry = Arc::new(support::memory_with(values.clone()));
    registry.get_all().write().unwrap().clear();
    assert!(
        slot.set(Arc::downgrade(&registry)).is_ok(),
        "registry bound once"
    );
    registry
}
#[test]
fn provider_key_samples_fallback_after_credential_effect() {
    if dynamic::isolated("provider_key_samples_fallback_after_credential_effect") {
        return;
    }
    for edit in ["replace", "remove", "add"] {
        let slot: Arc<std::sync::OnceLock<std::sync::Weak<ModelRegistry>>> = Arc::default();
        let values = reentrant(
            &slot,
            move |registry, _| {
                assert!(registry.get_error().is_none());
                if edit == "remove" {
                    registry.unregister_provider("p").unwrap();
                } else {
                    let config = dynamic::config(&["m"]);
                    config.write().unwrap().api_key = Some("later-key".into());
                    registry.register_provider("p", config).unwrap();
                }
            },
            None,
        );
        let registry = bind(&slot, &values);
        if edit != "add" {
            registry
                .register_provider("p", dynamic::config(&["m"]))
                .unwrap();
        }
        registry.auth_storage().set(
            "p",
            maestro_credentials::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
                env: None,
                key: format!("!stored-{edit}"),
            }),
        );
        maestro_credentials::clear_config_value_cache();
        let key = request_support::run(registry.get_api_key_for_provider("p")).unwrap();
        assert!(
            key.as_deref().eq(&if edit == "remove" {
                None
            } else {
                Some("later-key")
            }),
            "late configured fallback"
        );
        empty_key_blocks_fallback(&registry, &values.calls);
    }
}

/// Account hooks below the credential owner's extraction and metadata boundaries.
struct Account {
    inner: request_support::OAuth,
    name_effect: Box<dyn Fn() + Send + Sync>,
    key_effect: Box<dyn Fn() + Send + Sync>,
}
impl maestro_models::OAuthProviderInterface for Account {
    fn id(&self) -> &str {
        self.inner.id
    }
    fn name(&self) -> &str {
        (self.name_effect)();
        self.inner.name
    }
    fn login(
        &self,
        callbacks: maestro_models::OAuthCallbacks,
        fetch: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > {
        self.inner.login(callbacks, fetch)
    }
    fn refresh_token(
        &self,
        credentials: maestro_models::OAuthCredentials,
        fetch: Option<maestro_models::Fetch>,
    ) -> maestro_models::BoxFuture<
        Result<maestro_models::OAuthCredentials, maestro_models::OAuthError>,
    > {
        self.inner.refresh_token(credentials, fetch)
    }
    fn get_api_key<'a>(
        &self,
        credentials: &'a maestro_models::OAuthCredentials,
    ) -> Result<std::borrow::Cow<'a, str>, maestro_models::OAuthError> {
        (self.key_effect)();
        self.inner.get_api_key(credentials)
    }
}
#[test]
fn credential_and_metadata_effects_can_reenter_catalog() {
    if dynamic::isolated("credential_and_metadata_effects_can_reenter_catalog") {
        return;
    }
    let slot: Arc<std::sync::OnceLock<std::sync::Weak<ModelRegistry>>> = Arc::default();
    let values = reentrant(
        &slot,
        |registry, _| {
            registry
                .register_provider("configuration", dynamic::config(&["nested"]))
                .unwrap();
            assert!(registry.find("fallback", "nested").is_some());
        },
        Some("resolved"),
    );
    let registry = bind(&slot, &values);
    extraction_reenters(&registry);
    let weak = Arc::downgrade(&registry);
    let visits = Arc::new(Mutex::new(Vec::new()));
    let seen = visits.clone();
    registry
        .auth_storage()
        .set_fallback_resolver(move |provider| {
            let registry = weak.upgrade().unwrap();
            let config = dynamic::config(&["nested"]);
            config.write().unwrap().api_key = Some("!configured-helper".into());
            registry.register_provider("fallback", config).unwrap();
            seen.lock().unwrap().push(provider.to_owned());
            Ok(None)
        });
    registry
        .register_provider("metadata", dynamic::config(&["m"]))
        .unwrap();
    let model = registry.find("metadata", "m").unwrap();
    assert!(registry.has_configured_auth(&model).unwrap());
    assert!(
        registry
            .get_provider_auth_status("metadata")
            .unwrap()
            .configured
    );
    *registry.get_all().write().unwrap() = vec![model.clone()];
    assert!(Arc::ptr_eq(&registry.get_available().unwrap()[0], &model));
    assert_eq!(
        *visits.lock().unwrap(),
        ["metadata", "metadata", "metadata"]
    );
    let key = request_support::run(registry.get_api_key_for_provider("fallback")).unwrap();
    assert!(
        key.as_deref().eq(&Some("resolved")),
        "configured command result"
    );
    assert!(registry.find("configuration", "nested").is_some());
    metadata_error_stops_effects(&registry);
}

#[test]
fn display_name_effect_can_reenter_catalog() {
    if dynamic::isolated("display_name_effect_can_reenter_catalog") {
        return;
    }
    let registry = registry();
    let weak = Arc::downgrade(&registry);
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(Account {
        inner: request_support::OAuth::new("p", "Retained"),
        name_effect: Box::new(move || {
            let registry = weak.upgrade().unwrap();
            assert!(registry.find("p", "m").is_some());
            registry
                .register_provider("name-effect", dynamic::config(&["nested"]))
                .unwrap();
        }),
        key_effect: Box::new(|| {}),
    }));
    registry.register_provider("p", config).unwrap();
    let _current = request_support::OAuth::new("p", "Current").register();
    assert_eq!(registry.get_provider_display_name("p"), "Retained");
    assert!(registry.find("name-effect", "nested").is_some());
    registry
        .register_provider(
            "p",
            Arc::new(std::sync::RwLock::new(
                maestro_catalog::ProviderConfigInput {
                    name: Some(String::new()),
                    ..Default::default()
                },
            )),
        )
        .unwrap();
    assert_eq!(registry.get_provider_display_name("p"), "");
}

/// File observations whose callbacks are enabled only after construction.
#[derive(Default)]
struct Files {
    registry: Mutex<std::sync::Weak<ModelRegistry>>,
    mode: std::sync::atomic::AtomicUsize,
    events: Mutex<Vec<&'static str>>,
}
impl Files {
    /// Observe and re-enter without retaining a fixture guard.
    fn effect(&self, phase: &'static str) {
        self.events.lock().unwrap().push(phase);
        let registry = self.registry.lock().unwrap().upgrade();
        if let Some(registry) = registry {
            assert!(
                registry
                    .get_all()
                    .read()
                    .unwrap()
                    .iter()
                    .all(|m| !m.read().unwrap().id.is_empty())
            );
            registry
                .register_provider(phase, dynamic::config(&[phase]))
                .unwrap();
        }
    }
}
/// File-effect adapter retaining the caller-observed fixture.
struct FileEffects(Arc<Files>);
impl maestro_catalog::ModelFileOperations for FileEffects {
    fn exists(&self, _: &str) -> bool {
        self.0.effect("exists");
        self.0.mode.load(std::sync::atomic::Ordering::SeqCst) != 0
    }
    fn read_to_string(&self, _: &str) -> std::io::Result<String> {
        self.0.effect("read");
        match self.0.mode.load(std::sync::atomic::Ordering::SeqCst) {
            1 => Ok("{\"providers\":{}}".into()),
            2 => Err(std::io::Error::other("first read failure")),
            _ => Err(std::io::Error::other("second read failure")),
        }
    }
}
/// Real file-loading route with callbacks enabled after ownership is established.
fn file_registry() -> (Arc<ModelRegistry>, Arc<Files>) {
    let files = Arc::new(Files::default());
    let registry = Arc::new(
        ModelRegistry::with_operations(
            support::auth(),
            "models",
            FileEffects(files.clone()),
            crate::support::values(),
        )
        .unwrap(),
    );
    *files.registry.lock().unwrap() = Arc::downgrade(&registry);
    files.events.lock().unwrap().clear();
    (registry, files)
}
#[test]
fn file_effects_can_reenter_refresh() {
    if dynamic::isolated("file_effects_can_reenter_refresh") {
        return;
    }
    for mode in [0, 1, 2] {
        let (registry, files) = file_registry();
        files.mode.store(mode, std::sync::atomic::Ordering::SeqCst);
        registry.refresh().unwrap();
        assert_eq!(
            *files.events.lock().unwrap(),
            if mode == 0 {
                vec!["exists"]
            } else {
                vec!["exists", "read"]
            }
        );
        assert!(registry.find("exists", "exists").is_some());
        assert_eq!(registry.find("read", "read").is_some(), mode != 0);
        assert_eq!(registry.get_error().is_some(), mode == 2);
    }
}

/// Re-entrant final-alias cleanup, using weak catalog ownership.
struct Cleanup {
    registry: std::sync::Weak<ModelRegistry>,
    events: Arc<Mutex<Vec<String>>>,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(registry) = self.registry.upgrade() {
            self.events
                .lock()
                .unwrap()
                .push(registry.get_provider_display_name("p"));
            registry
                .register_provider("cleanup", dynamic::config(&["released"]))
                .unwrap();
        }
    }
}
#[test]
fn retired_catalog_callbacks_can_reenter_after_removal() {
    if dynamic::isolated("retired_catalog_callbacks_can_reenter_after_removal") {
        return;
    }
    for mode in ["replace", "remove", "retained"] {
        let registry = registry();
        let events = Arc::new(Mutex::new(Vec::new()));
        let cleanup = Cleanup {
            registry: Arc::downgrade(&registry),
            events: events.clone(),
        };
        let account: maestro_models::OAuthProviderHandle = Arc::new(Account {
            inner: request_support::OAuth::new("p", "Original"),
            name_effect: Box::new(move || {
                let _ = &cleanup;
            }),
            key_effect: Box::new(|| {}),
        });
        let retained = (mode == "retained").then(|| account.clone());
        let config = dynamic::config(&["m"]);
        config.write().unwrap().callbacks.oauth = Some(account);
        registry.register_provider("p", config).unwrap();
        if mode == "replace" {
            let replacement = dynamic::config(&["new"]);
            replacement.write().unwrap().callbacks.oauth = Some(dynamic::oauth("Replacement"));
            registry.register_provider("p", replacement).unwrap();
            assert_eq!(*events.lock().unwrap(), ["Replacement"]);
        } else {
            registry.unregister_provider("p").unwrap();
            assert!(registry.find("p", "m").is_none());
            if let Some(account) = retained {
                assert!(events.lock().unwrap().is_empty());
                assert_eq!(account.name(), "Original");
                drop(account);
            }
            assert_eq!(*events.lock().unwrap(), ["p"]);
        }
        assert!(registry.find("cleanup", "released").is_some());
        maestro_models::reset_oauth_providers();
    }
}

#[test]
fn load_diagnostic_is_owned_across_refresh() {
    if dynamic::isolated("load_diagnostic_is_owned_across_refresh") {
        return;
    }
    let (registry, files) = file_registry();
    files.mode.store(2, std::sync::atomic::Ordering::SeqCst);
    registry.refresh().unwrap();
    let first: String = registry.get_error().unwrap();
    files.mode.store(1, std::sync::atomic::Ordering::SeqCst);
    registry.refresh().unwrap();
    assert!(registry.get_error().is_none());
    files.mode.store(3, std::sync::atomic::Ordering::SeqCst);
    registry.refresh().unwrap();
    assert!(
        registry
            .get_error()
            .unwrap()
            .contains("second read failure")
    );
    assert!(first.contains("first read failure"));
}

#[test]
fn native_shared_catalog_and_local_action_both_work() {
    if dynamic::isolated("native_shared_catalog_and_local_action_both_work") {
        return;
    }
    thread_safe::<ModelRegistry>();
    thread_safe::<Arc<ModelRegistry>>();
    let registry = registry();
    let worker = registry.clone();
    std::thread::spawn(move || {
        worker
            .register_provider("thread", dynamic::config(&["m"]))
            .unwrap();
        worker.refresh().unwrap();
    })
    .join()
    .unwrap();
    assert!(registry.find("thread", "m").is_some());
    let local = std::rc::Rc::new(
        ModelRegistry::in_memory(support::auth(), crate::support::values()).unwrap(),
    );
    let count = std::rc::Rc::new(std::cell::Cell::new(0));
    let action = {
        let local = local.clone();
        let count = count.clone();
        move || {
            count.set(count.get() + 1);
            local.register_provider("local", dynamic::config(&["m"]))
        }
    };
    action().unwrap();
    local.refresh().unwrap();
    assert!(local.find("local", "m").is_some());
    assert_eq!(count.get(), 1);
}

/// Apply one recorded callback edit to its selected list.
fn edit_selection(
    registry: &ModelRegistry,
    edit: &str,
    models: &mut Vec<std::borrow::Cow<'_, maestro_models::Model>>,
) -> Result<(), maestro_models::OAuthError> {
    match edit {
        "mutate" => {
            registry.find("outer", "one").unwrap().write().unwrap().name = "edited".into();
        }
        "replace" => {
            let mut model = models[0].clone().into_owned();
            model.id = "replacement".into();
            model.name = "replacement".into();
            *models = vec![std::borrow::Cow::Owned(model)];
        }
        "add" => {
            let mut model = models[0].clone().into_owned();
            model.id = "added".into();
            model.name = "added".into();
            models.push(std::borrow::Cow::Owned(model));
        }
        "delete" => {
            models.remove(0);
        }
        "empty" => models.clear(),
        "error" => return Err(dynamic::account_failure("selection refused")),
        _ => panic!("unknown recorded edit"),
    }
    Ok(())
}

/// An empty stored value is present, never a configured fallback request.
fn empty_key_blocks_fallback(registry: &ModelRegistry, commands: &Mutex<Vec<String>>) {
    registry.auth_storage().set(
        "p",
        maestro_credentials::AuthCredential::ApiKey(maestro_credentials::ApiKeyCredential {
            env: None,
            key: String::new(),
        }),
    );
    let asked = commands.lock().unwrap().len();
    let key = request_support::run(registry.get_api_key_for_provider("p")).unwrap();
    assert!(key.as_deref().eq(&Some("")), "present empty credential");
    assert_eq!(
        commands.lock().unwrap().len(),
        asked,
        "empty stored credential must not resolve fallback"
    );
}

/// Credential extraction invokes caller code without a catalog guard.
fn extraction_reenters(registry: &Arc<ModelRegistry>) {
    let weak = Arc::downgrade(registry);
    let account = Account {
        inner: request_support::OAuth::new("p", "Provider"),
        name_effect: Box::new(|| {}),
        key_effect: Box::new(move || {
            let registry = weak.upgrade().unwrap();
            assert!(registry.find("p", "m").is_some());
            registry
                .register_provider("extraction", dynamic::config(&["nested"]))
                .unwrap();
        }),
    };
    let config = dynamic::config(&["m"]);
    config.write().unwrap().callbacks.oauth = Some(Arc::new(account));
    registry.register_provider("p", config).unwrap();
    registry
        .auth_storage()
        .set("p", request_support::tokens(f64::MAX));
    request_support::success(
        request_support::request(registry),
        Some("unused-access"),
        &[],
    );
    assert!(registry.find("extraction", "nested").is_some());
}

/// A credential error ends provider auth status with its fallback diagnostic.
fn metadata_error_stops_effects(registry: &ModelRegistry) {
    registry
        .auth_storage()
        .set_fallback_resolver(|_| Err(dynamic::account_failure("fallback refused")));
    dynamic::assert_account_failure(
        &registry.get_provider_auth_status("metadata").unwrap_err(),
        "fallback refused",
    );
}

/// Native callers may share the catalog directly.
fn thread_safe<T: Send + Sync>() {}

/// Observe retained insertion order through the next public replay, not private state.
fn assert_retained_order(
    registry: &ModelRegistry,
    handles: &[(
        &'static str,
        std::sync::Weak<std::sync::RwLock<maestro_catalog::ProviderConfigInput>>,
    )],
    expected: &[String],
) {
    let events = Arc::new(Mutex::new(Vec::new()));
    for (name, handle) in handles {
        registry
            .auth_storage()
            .set(name, request_support::tokens(f64::MAX));
        if let Some(config) = handle.upgrade() {
            config.write().unwrap().callbacks.oauth = recording(&events, name)
                .read()
                .unwrap()
                .callbacks
                .oauth
                .clone();
        }
    }
    registry.refresh().unwrap();
    assert_eq!(*events.lock().unwrap(), expected);
}
