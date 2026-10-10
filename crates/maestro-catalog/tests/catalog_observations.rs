#![cfg(test)]
//! Shared catalog state and reload observations.
mod support;
use maestro_catalog::ModelRegistry;
use maestro_models::get_model;
use std::sync::{Arc, Mutex, RwLock};

#[test]
fn reload_replaces_custom_data_and_clears_load_errors() {
    support::corpus("reload_replaces_custom_data_and_clears_load_errors");
    let original = get_model("openrouter", "anthropic/claude-sonnet-4").unwrap();
    for native in [false, true] {
        reload(native, &original);
    }
}
/// Exercise the same reload sequence on either file adapter.
fn reload(native: bool, original: &Model) {
    let initial = r#"{"providers":{"openrouter":{"baseUrl":"first","modelOverrides":{"anthropic/claude-sonnet-4":{"name":"First"}},"models":[{"id":"old"}]}}}"#;
    let file = support::File::new(initial.as_bytes());
    let text = support::Text(Arc::new(Mutex::new(Some(initial.into()))));
    let mut registry = if native {
        ModelRegistry::create(support::auth(), file.path()).unwrap()
    } else {
        ModelRegistry::with_operations(support::auth(), "<models-file>", text.clone()).unwrap()
    };
    assert_eq!(
        registry
            .find("openrouter", "anthropic/claude-sonnet-4")
            .unwrap()
            .read()
            .unwrap()
            .name,
        "First"
    );
    for (input, name, error) in [
        (
            Some(
                r#"{"providers":{"openrouter":{"baseUrl":"second","modelOverrides":{"anthropic/claude-sonnet-4":{"name":"Second"}},"models":[{"id":"new"}]}}}"#,
            ),
            "Second",
            false,
        ),
        (Some("{"), original.name.as_str(), true),
        (Some(r#"{"providers":{}}"#), original.name.as_str(), false),
        (None, original.name.as_str(), false),
    ] {
        if native {
            match input {
                Some(input) => std::fs::write(file.path(), input).unwrap(),
                None => std::fs::remove_file(file.path()).unwrap(),
            }
        } else {
            *text.0.lock().unwrap() = input.map(str::to_owned);
        }
        registry.refresh().unwrap();
        let model = registry
            .find("openrouter", "anthropic/claude-sonnet-4")
            .unwrap();
        assert_eq!(model.read().unwrap().name, name);
        assert_eq!(registry.get_error().is_some(), error);
        assert!(registry.find("openrouter", "old").is_none());
        assert_eq!(
            registry.find("openrouter", "new").is_some(),
            name == "Second"
        );
        assert_eq!(
            model.read().unwrap().base_url,
            if name == "Second" {
                "second"
            } else {
                &original.base_url
            }
        );
    }
}

#[test]
fn retained_catalog_handles_track_edits_and_survive_reload() {
    let mut registry = ModelRegistry::in_memory(support::auth()).unwrap();
    let old = registry.get_all();
    assert!(Arc::ptr_eq(&old, &registry.get_all()));
    let first = old.read().unwrap()[0].clone();
    let (provider, id) = {
        let m = first.read().unwrap();
        (m.provider.clone(), m.id.clone())
    };
    assert!(Arc::ptr_eq(&first, &registry.find(&provider, &id).unwrap()));
    first.write().unwrap().name = "Alias edit".into();
    assert_eq!(
        registry.find(&provider, &id).unwrap().read().unwrap().name,
        "Alias edit"
    );
    old.write().unwrap().reverse();
    assert!(Arc::ptr_eq(
        &registry.get_all().read().unwrap()[969],
        &first
    ));
    let mut inserted = get_model(&provider, &id).unwrap();
    inserted.id = "inserted".into();
    let inserted = Arc::new(RwLock::new(inserted));
    old.write().unwrap().push(inserted.clone());
    assert!(Arc::ptr_eq(
        &registry.find(&provider, "inserted").unwrap(),
        &inserted
    ));
    old.write().unwrap().retain(|m| !Arc::ptr_eq(m, &first));
    assert!(registry.find(&provider, &id).is_none());
    old.write().unwrap().insert(0, first.clone());
    let weak = Arc::downgrade(&first);
    registry.refresh().unwrap();
    assert!(!Arc::ptr_eq(&old, &registry.get_all()));
    assert_eq!(first.read().unwrap().name, "Alias edit");
    assert!(
        old.read()
            .unwrap()
            .iter()
            .any(|m| Arc::ptr_eq(m, &inserted))
    );
    drop(first);
    assert!(weak.upgrade().is_some());
    drop(old);
    assert!(weak.upgrade().is_none());
}

#[test]
fn exact_find_preserves_case_provider_and_descriptor_identity() {
    support::corpus("exact_find_preserves_case_provider_and_descriptor_identity");
    let registry = support::registry(
        r#"{"providers":{"a:b":{"baseUrl":"u","apiKey":"unused","api":"a","models":[{"id":"c","name":"first"}]},"a":{"baseUrl":"u","apiKey":"unused","api":"a","models":[{"id":"b:c","name":"second"},{"id":"Case"},{"id":"case"},{"id":"space / model"}]}}}"#,
    );
    for (provider, id, name) in [
        ("a:b", "c", "first"),
        ("a", "b:c", "second"),
        ("a", "Case", "Case"),
        ("a", "case", "case"),
        ("a", "space / model", "space / model"),
    ] {
        let model = registry.find(provider, id).unwrap();
        assert_eq!(model.read().unwrap().name, name);
        assert!(Arc::ptr_eq(&model, &registry.find(provider, id).unwrap()));
    }
    for (provider, id) in [
        ("A", "case"),
        ("a", "CASE"),
        ("a", "space"),
        ("unknown", "case"),
    ] {
        assert!(registry.find(provider, id).is_none());
    }
}
use maestro_credentials::{ApiKeyCredential, AuthCredential};
use maestro_models::{
    BoxFuture, Fetch, Model, OAuthCallbacks, OAuthCredentials, OAuthError, OAuthProviderInterface,
    register_oauth_provider, unregister_oauth_provider,
};
/// Controlled descriptor transformation.
type Transform = dyn Fn(Vec<Model>) -> Result<Vec<Model>, OAuthError> + Send + Sync;
/// Controlled transform without network or interaction.
struct Modifier {
    id: &'static str,
    action: Box<Transform>,
}
impl OAuthProviderInterface for Modifier {
    fn id(&self) -> &str {
        self.id
    }
    fn name(&self) -> &str {
        self.id
    }
    fn login(
        &self,
        _: OAuthCallbacks,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { panic!("unexpected login") })
    }
    fn refresh_token(
        &self,
        _: OAuthCredentials,
        _: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        Box::pin(async { panic!("unexpected refresh") })
    }
    fn get_api_key<'a>(&self, _: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        panic!("unexpected key resolution")
    }
    fn modify_models(
        &self,
        models: Vec<Model>,
        _: &OAuthCredentials,
    ) -> Result<Vec<Model>, OAuthError> {
        (self.action)(models)
    }
}
/// Registration cleanup even when an assertion fails.
struct Registrations(Vec<&'static str>);
impl Drop for Registrations {
    fn drop(&mut self) {
        for id in &self.0 {
            unregister_oauth_provider(id);
        }
    }
}
/// Register one controlled transformation.
fn register(
    id: &'static str,
    action: impl Fn(Vec<Model>) -> Result<Vec<Model>, OAuthError> + Send + Sync + 'static,
) {
    register_oauth_provider(Arc::new(Modifier {
        id,
        action: Box::new(action),
    }));
}
/// Complete tokens whose expiry is intentionally in the past.
fn tokens() -> AuthCredential {
    AuthCredential::OAuth(OAuthCredentials {
        refresh: "unused-refresh".into(),
        access: "unused-access".into(),
        expires: 0.0,
        extra: maestro_models::JsonObject::new(),
    })
}
#[test]
fn oauth_transforms_run_in_registration_order_for_stored_tokens() {
    let ids = [
        "catalog-order-a",
        "catalog-order-b",
        "catalog-order-key",
        "catalog-order-missing",
    ];
    let _registrations = Registrations(ids.to_vec());
    let auth = support::auth();
    let calls = Arc::new(Mutex::new(Vec::new()));
    order_modifiers(&ids, &calls);
    for id in &ids[..2] {
        auth.set(id, tokens());
    }
    auth.set(
        ids[2],
        AuthCredential::ApiKey(ApiKeyCredential {
            key: "unused".into(),
        }),
    );
    let original: Vec<_> = maestro_models::get_providers()
        .iter()
        .flat_map(|p| maestro_models::get_models(p))
        .map(|m| (m.provider, m.id))
        .collect();
    let mut registry = ModelRegistry::in_memory(auth).unwrap();
    for iteration in 0..2 {
        if iteration == 1 {
            registry.refresh().unwrap();
        }
        let all = registry.get_all();
        let all = all.read().unwrap();
        assert_eq!(all[0].read().unwrap().name, "AB");
        assert_eq!(
            all.iter()
                .map(|m| {
                    let m = m.read().unwrap();
                    (m.provider.clone(), m.id.clone())
                })
                .collect::<Vec<_>>(),
            original.iter().rev().cloned().collect::<Vec<_>>()
        );
    }
    assert_eq!(*calls.lock().unwrap(), ["a", "b", "a", "b"]);
}
#[test]
fn failed_oauth_transform_preserves_published_catalog() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let ids = ["catalog-fail-first", "catalog-fail-later"];
    let _registrations = Registrations(ids.to_vec());
    let fail = Arc::new(AtomicBool::new(false));
    let observed = fail.clone();
    register(ids[0], move |models| {
        if observed.load(Ordering::SeqCst) {
            Err(maestro_models::DiagnosticErrorInfo {
                name: None,
                message: "transform failed".into(),
                stack: None,
                code: None,
            }
            .into())
        } else {
            Ok(models)
        }
    });
    let later = Arc::new(Mutex::new(0));
    let observed = later.clone();
    register(ids[1], move |models| {
        *observed.lock().unwrap() += 1;
        Ok(models)
    });
    let auth = support::auth();
    for id in ids {
        auth.set(id, tokens());
    }
    let text = support::Text(Arc::new(Mutex::new(Some("{".into()))));
    let mut registry =
        ModelRegistry::with_operations(auth.clone(), "<models-file>", text.clone()).unwrap();
    assert!(registry.get_error().is_some());
    let old = registry.get_all();
    *text.0.lock().unwrap() =
        Some(r#"{"providers":{"openrouter":{"models":[{"id":"new"}]}}}"#.into());
    fail.store(true, Ordering::SeqCst);
    assert_eq!(
        registry.refresh().unwrap_err().to_string(),
        "transform failed"
    );
    assert!(Arc::ptr_eq(&old, &registry.get_all()));
    assert_eq!(registry.get_error(), None);
    assert!(registry.find("openrouter", "new").is_none());
    assert_eq!(*later.lock().unwrap(), 1);
    let result = ModelRegistry::in_memory(auth);
    assert_eq!(result.err().unwrap().to_string(), "transform failed");
    assert_eq!(*later.lock().unwrap(), 1);
}

/// Register a stateful ordered chain plus key-only and absent-credential decoys.
fn order_modifiers(ids: &[&'static str; 4], calls: &Arc<Mutex<Vec<&'static str>>>) {
    let observed = calls.clone();
    register(ids[0], move |mut models| {
        observed.lock().unwrap().push("a");
        models.reverse();
        models[0].name = "A".into();
        Ok(models)
    });
    let observed = calls.clone();
    register(ids[1], move |mut models| {
        observed.lock().unwrap().push("b");
        assert_eq!(models[0].name, "A");
        models[0].name.push('B');
        Ok(models)
    });
    for id in &ids[2..] {
        let observed = calls.clone();
        register(id, move |models| {
            observed.lock().unwrap().push("unexpected");
            Ok(models)
        });
    }
}
