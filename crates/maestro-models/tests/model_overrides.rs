mod support;
use maestro_models::*;
use std::sync::Arc;
use support::catalog::*;
use support::{
    auth::{headers, local},
    block_on,
    conformance::{context, done, steps},
};

#[test]
fn endpoint_override_retains_catalog_and_other_providers() {
    let adapter = Arc::new(RecordingProvider::default());
    let mut models = models();
    let chat = entry("local", "base", "chat");
    let native = entry("local", "base", "embedding");
    let other = entry("other", "base", "chat");
    for model in [&chat, &native, &other] {
        models.register(model.clone(), adapter.clone()).unwrap();
    }
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: Some("new:endpoint".into()),
                models: vec![],
            },
        )
        .unwrap();
    assert_eq!(models.known(None).len(), 3);
    for original in [&chat, &native] {
        let mut expected = original.clone();
        expected.endpoint = "new:endpoint".into();
        assert_eq!(models.find(&original.identity), Some(expected));
    }
    assert_eq!(models.find(&other.identity), Some(other.clone()));
    assert_eq!(
        block_on(models.complete(chat.clone(), context(), local())).failure,
        None
    );
    let calls = adapter.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0.endpoint, "new:endpoint");
}

#[test]
fn model_override_is_operation_qualified_and_wins_endpoint_precedence() {
    let adapter = Arc::new(RecordingProvider::default());
    let mut models = models();
    let mut chat = entry("local", "same", "chat");
    chat.headers = headers(&[("x-base", "literal")]);
    let native = entry("local", "same", "embedding");
    models
        .register_catalog(
            "local",
            || Ok(vec![chat.clone(), native.clone()]),
            adapter.clone(),
        )
        .unwrap();
    let mut changed = chat.clone();
    changed.name = "changed".into();
    changed.rates.get_or_insert_with(TokenRates::default).input = 3.5;

    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: Some("provider:endpoint".into()),
                models: vec![changed.clone()],
            },
        )
        .unwrap();
    assert_eq!(models.find(&chat.identity), Some(changed.clone()));
    let mut expected_native = native.clone();
    expected_native.endpoint = "provider:endpoint".into();
    assert_eq!(models.find(&native.identity), Some(expected_native));
    assert_eq!(
        block_on(models.complete(chat, context(), local())).failure,
        None
    );
    assert_eq!(adapter.calls.lock().unwrap()[0].0, changed);
}

#[test]
fn unmatched_override_waits_for_a_matching_registration() {
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let mut models = models();
    let base = entry("local", "base", "chat");
    let later = entry("local", "later", "chat");
    let mut changed = later.clone();
    changed.endpoint = "later:override".into();
    models.register(base.clone(), fake.clone()).unwrap();
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: None,
                models: vec![changed.clone()],
            },
        )
        .unwrap();
    assert_eq!(models.known(None), vec![base.clone()]);
    assert_eq!(models.find(&later.identity), None);
    assert_eq!(
        block_on(models.complete(later.clone(), context(), local())).failure,
        Some(Failure::UnknownModel)
    );
    assert!(fake.calls().is_empty());
    models.register(later.clone(), fake.clone()).unwrap();
    assert_eq!(models.find(&later.identity), Some(changed.clone()));
    assert_eq!(
        block_on(models.complete(later.clone(), context(), local())).failure,
        None
    );
    assert_eq!(fake.calls()[0].model, changed.clone());
    models
        .register_catalog("local", || Ok(vec![base]), fake.clone())
        .unwrap();
    assert_eq!(models.find(&later.identity), None);
    models
        .register_catalog("local", || Ok(vec![]), fake.clone())
        .unwrap();
    models.register(later.clone(), fake).unwrap();
    assert_eq!(models.find(&later.identity), Some(changed));
}

#[test]
fn invalid_override_preserves_previous_effective_catalog() {
    let adapter = Arc::new(RecordingProvider::default());
    let mut models = models();
    let base = entry("local", "base", "chat");
    models.register(base.clone(), adapter.clone()).unwrap();
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: Some("previous:endpoint".into()),
                models: vec![],
            },
        )
        .unwrap();
    let before = models.find(&base.identity).unwrap();
    let unmatched = entry("local", "unmatched", "chat");
    let mut bad = unmatched.clone();
    bad.rates
        .get_or_insert_with(TokenRates::default)
        .cache_write = f64::NAN;
    let mut wrong = unmatched.clone();
    wrong.identity.provider = "wrong".into();
    for (overrides, failure) in [
        (
            CatalogOverride {
                endpoint: Some(String::new()),
                models: vec![],
            },
            Failure::InvalidCatalog,
        ),
        (
            CatalogOverride {
                endpoint: None,
                models: vec![unmatched.clone(), bad],
            },
            Failure::InvalidCatalog,
        ),
        (
            CatalogOverride {
                endpoint: None,
                models: vec![wrong],
            },
            Failure::InvalidCatalog,
        ),
        (
            CatalogOverride {
                endpoint: None,
                models: vec![unmatched.clone(), unmatched],
            },
            Failure::DuplicateModel,
        ),
    ] {
        assert_eq!(models.set_override("local", overrides), Err(failure));
        assert_eq!(models.find(&base.identity), Some(before.clone()));
    }
    assert_eq!(
        models.set_override("unknown", CatalogOverride::default()),
        Err(Failure::UnknownProvider)
    );
    let mut invalid_base = base.clone();
    invalid_base.endpoint.clear();
    assert_eq!(
        models.register_catalog("local", || Ok(vec![invalid_base]), adapter.clone()),
        Err(Failure::InvalidCatalog)
    );
    assert_eq!(models.find(&base.identity), Some(before.clone()));
    assert_eq!(
        block_on(models.complete(base, context(), local())).failure,
        None
    );
    assert_eq!(adapter.calls.lock().unwrap()[0].0, before);
}

#[test]
fn removing_override_restores_latest_registration() {
    let old = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let next = Arc::new(RecordingProvider::default());
    let mut models = models();
    let base = entry("local", "base", "chat");
    let other = entry("other", "base", "chat");
    models.register(base.clone(), old.clone()).unwrap();
    models.register(other.clone(), old.clone()).unwrap();
    let mut replacement = base.clone();
    replacement.endpoint = "full:override".into();
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: Some("provider:override".into()),
                models: vec![replacement.clone()],
            },
        )
        .unwrap();
    let mut latest = base.clone();
    latest.protocol = "latest:protocol".into();
    latest.endpoint = "latest:endpoint".into();
    latest.headers = headers(&[("x-latest", "latest")]);
    latest.rates.get_or_insert_with(TokenRates::default).output = 2.0;

    models
        .register_catalog("local", || Ok(vec![latest.clone()]), next.clone())
        .unwrap();
    assert_eq!(models.find(&base.identity), Some(replacement));
    assert!(models.remove_override("local"));
    assert!(!models.remove_override("local"));
    assert_eq!(models.find(&base.identity), Some(latest.clone()));
    assert_eq!(models.find(&other.identity), Some(other));
    assert_eq!(
        block_on(models.complete(latest.clone(), context(), local())).failure,
        None
    );
    assert_eq!(next.calls.lock().unwrap()[0].0, latest);
    assert!(old.calls().is_empty());
}
