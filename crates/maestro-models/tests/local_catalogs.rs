mod support;
use maestro_models::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use support::{
    auth::*,
    block_on,
    conformance::{context, done, steps},
};

use support::catalog::{Getter, entry};

#[test]
fn known_lookup_is_offline_operation_qualified_and_owned() {
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let resolver = Resolver::new(vec![]);

    let mut models = Models::new(Arc::new(|| 73));
    let mut chat = entry("opaque:provider/path", "id:with/slash", "chat");
    chat.input = vec!["opaque:input/path".into()];
    let other = entry(
        &chat.identity.provider,
        &chat.identity.model,
        "native:other/op",
    );
    let getter = Getter {
        calls: AtomicUsize::new(0),
        result: Ok(vec![chat.clone(), other.clone()]),
    };
    models
        .register_catalog(&chat.identity.provider, || getter.get(), fake.clone())
        .unwrap();
    assert_eq!(models.known(None).len(), 2);
    assert_eq!(models.known(Some("chat")), vec![chat.clone()]);
    assert_eq!(models.find(&other.identity), Some(other));
    let mut owned = models.find(&chat.identity).unwrap();
    owned.endpoint = "changed".into();
    owned.input.clear();
    assert_eq!(models.find(&chat.identity), Some(chat));
    assert_eq!(getter.calls.load(Ordering::SeqCst), 1);
    assert_eq!(resolver.count(), 0);
    assert_eq!(resolver.statuses.load(Ordering::SeqCst), 0);
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
}

#[test]
fn available_lookup_uses_configured_metadata_and_selected_account_filter() {
    let exchange = Arc::new(Exchange {
        calls: AtomicUsize::new(0),
        gate: None,
        failure: false,
    });
    let adapter = Arc::new(Adapter {
        exchange: Some(exchange.clone()),
        ..Default::default()
    });
    let mut models = Models::new(Arc::new(|| 73));
    let a = entry("local", "allowed", "chat");
    let b = entry("local", "blocked", "chat");
    let c = entry("unconfigured", "other", "chat");
    let configured_secret = entry("configured-secret", "secret", "chat");
    for model in [&a, &b, &c, &configured_secret] {
        models.register(model.clone(), adapter.clone()).unwrap();
    }
    struct Statuses {
        samples: std::sync::Mutex<Vec<String>>,
    }
    impl AuthResolver for Statuses {
        fn status(&self, p: &str) -> AuthStatus {
            self.samples.lock().unwrap().push(p.into());
            AuthStatus {
                configured: p != "unconfigured",
                source: Some(
                    if p == "local" {
                        "explicit secret-free"
                    } else {
                        "credential metadata"
                    }
                    .into(),
                ),
            }
        }
        fn resolve(
            &self,
            _: String,
            _: Cancellation,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<RequestAuth, Failure>> + Send + '_>,
        > {
            panic!("lookup resolved auth")
        }
    }
    let resolver = Statuses {
        samples: Default::default(),
    };
    let predicates = AtomicUsize::new(0);
    let found = models.available(Some("chat"), &resolver, &|m| {
        assert_ne!(m.identity.provider, "unconfigured");
        predicates.fetch_add(1, Ordering::SeqCst);
        m.identity.model == "allowed"
    });
    assert_eq!(predicates.load(Ordering::SeqCst), 3);
    assert_eq!(
        found,
        vec![AvailableModel {
            model: a.clone(),
            auth_status: AuthStatus {
                configured: true,
                source: Some("explicit secret-free".into())
            }
        }]
    );
    let mut samples = resolver.samples.lock().unwrap().clone();
    samples.sort();
    assert_eq!(samples, ["configured-secret", "local", "unconfigured"]);
    let unrestricted = models.available(None, &resolver, &|_| true);
    assert_eq!(unrestricted.len(), 3);
    assert!(unrestricted.contains(&AvailableModel {
        model: configured_secret,
        auth_status: AuthStatus {
            configured: true,
            source: Some("credential metadata".into())
        }
    }));
    assert_eq!(models.known(None).len(), 4);
    assert_eq!(exchange.calls.load(Ordering::SeqCst), 0);
    assert!(adapter.calls.lock().unwrap().is_empty());
    let failing = Arc::new(Resolver::new(vec![Err(Failure::AuthenticationFailed)]));
    let result = block_on(models.complete(
        found[0].model.clone(),
        context(),
        resolving(failing.clone()),
    ));
    assert_eq!(result.failure, Some(Failure::AuthenticationFailed));
    assert_eq!(failing.count(), 1);
    assert!(adapter.calls.lock().unwrap().is_empty());
    assert_eq!(exchange.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn catalog_getter_failure_reports_without_erasing_usable_entries() {
    let original = Arc::new(Adapter::default());
    let replacement = Arc::new(Adapter::default());
    let mut models = Models::new(Arc::new(|| 73));
    let base = entry("local", "base", "chat");
    let other = entry("other", "base", "chat");
    models.register(other.clone(), original.clone()).unwrap();
    let calls = AtomicUsize::new(0);
    let failed = || {
        calls.fetch_add(1, Ordering::SeqCst);
        Err(Failure::Transport)
    };
    assert_eq!(
        models.register_catalog("local", failed, replacement.clone()),
        Err(Failure::CatalogFailed)
    );
    assert_eq!(models.known(None), vec![other.clone()]);
    models
        .register_catalog(
            "local",
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(vec![base.clone()])
            },
            original.clone(),
        )
        .unwrap();
    assert_eq!(
        models.register_catalog("local", failed, replacement.clone()),
        Err(Failure::CatalogFailed)
    );
    assert_eq!(models.find(&base.identity), Some(base.clone()));
    assert_eq!(models.find(&other.identity), Some(other));
    assert_eq!(
        block_on(models.complete(base, context(), local())).failure,
        None
    );
    assert_eq!(original.calls.lock().unwrap().len(), 1);
    assert!(replacement.calls.lock().unwrap().is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert_eq!(
        Failure::CatalogFailed.to_string(),
        "local model catalog getter failed"
    );
}

#[test]
fn invalid_catalog_replacement_is_atomic() {
    let original = Arc::new(Adapter::default());
    let next = Arc::new(Adapter::default());
    let mut models = Models::new(Arc::new(|| 73));
    let base = entry("local", "base", "chat");
    let other = entry("other", "base", "chat");
    models.register(base.clone(), original.clone()).unwrap();
    models.register(other.clone(), original.clone()).unwrap();
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: Some("preserved:override".into()),
                models: vec![],
            },
        )
        .unwrap();
    let mut effective = base.clone();
    effective.endpoint = "preserved:override".into();
    let mut cases = Vec::new();
    for field in 0..9 {
        let mut bad = base.clone();
        match field {
            0 => bad.identity.provider.clear(),
            1 => bad.identity.model.clear(),
            2 => bad.identity.operation.clear(),
            3 => bad.protocol.clear(),
            4 => bad.endpoint.clear(),
            5 => bad.name.clear(),
            6 => bad.input = vec![String::new()],
            7 => bad.chat = None,
            _ => bad.identity.provider = "wrong".into(),
        }
        cases.push(bad);
    }
    for rate in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for field in 0..4 {
            let mut bad = base.clone();
            match field {
                0 => bad.rates.get_or_insert_with(TokenRates::default).input = rate,
                1 => bad.rates.get_or_insert_with(TokenRates::default).output = rate,
                2 => bad.rates.get_or_insert_with(TokenRates::default).cache_read = rate,
                _ => {
                    bad.rates
                        .get_or_insert_with(TokenRates::default)
                        .cache_write = rate
                }
            }
            cases.push(bad);
        }
    }
    for context_limit in [true, false] {
        let mut bad = base.clone();
        let chat = bad.chat.as_mut().unwrap();
        if context_limit {
            chat.context_window = Some(0);
        } else {
            bad.identity.model.clear();
        }
        cases.push(bad);
    }
    let mut wrong_chat = base.clone();
    wrong_chat.identity.operation = "embedding".into();
    cases.push(wrong_chat);
    let fresh = entry("local", "fresh", "chat");
    for bad in cases {
        assert_eq!(
            models.register_catalog(
                "local",
                || Ok(vec![fresh.clone(), bad.clone()]),
                next.clone()
            ),
            Err(Failure::InvalidCatalog)
        );
        assert_eq!(models.find(&base.identity), Some(effective.clone()));
        assert_eq!(models.find(&fresh.identity), None);
        if bad.identity.provider != "wrong" {
            let failure = if bad.rates.is_some() {
                Failure::DuplicateModel
            } else {
                Failure::InvalidCatalog
            };
            assert_eq!(models.register(bad, next.clone()), Err(failure));
            assert_eq!(models.find(&base.identity), Some(effective.clone()));
        }
    }
    assert_eq!(
        models.register_catalog(
            "local",
            || Ok(vec![fresh.clone(), fresh.clone()]),
            next.clone()
        ),
        Err(Failure::DuplicateModel)
    );
    assert_eq!(
        models.register_catalog("", || Ok(vec![]), next.clone()),
        Err(Failure::InvalidCatalog)
    );
    assert_eq!(
        block_on(models.complete(base.clone(), context(), local())).failure,
        None
    );
    assert_eq!(original.calls.lock().unwrap().len(), 1);
    assert_eq!(original.calls.lock().unwrap()[0].0, effective.clone());
    assert!(next.calls.lock().unwrap().is_empty());
    models
        .register_catalog("local", || Ok(vec![]), next)
        .unwrap();
    assert_eq!(models.find(&base.identity), None);
    assert_eq!(models.known(None), vec![other]);
    models.register(base.clone(), original).unwrap();
    assert_eq!(models.find(&base.identity), Some(effective));
}

#[test]
fn custom_metadata_defaults_are_fallbacks_not_measurements() {
    let mut chat = entry("local", "name", "chat");
    assert_eq!(chat.name, "name");
    assert_eq!(chat.input, ["text"]);
    assert!(chat.headers.is_empty());
    assert_eq!(chat.rates, None);
    assert_eq!(
        chat.chat,
        Some(ChatMetadata {
            context_window: Some(128_000),
        })
    );
    assert_eq!(chat.capabilities.output_limit, 16_384);
    assert!(!chat.capabilities.reasoning);
    assert!(chat.capabilities.thinking_level_map.is_empty());
    let native = entry("local", "native", "embedding");
    assert!(native.chat.is_none());
    assert!(native.input.is_empty());
    assert_eq!(native.rates, None);

    chat.rates = Some(TokenRates::default());
    chat.endpoint = "ENDPOINT_SENTINEL".into();
    chat.headers = headers(&[("x-key", "HEADER_SENTINEL")]);
    let metadata = chat.chat.as_mut().unwrap();
    metadata.context_window = None;
    chat.capabilities.output_limit = 7;
    chat.capabilities.reasoning = true;
    chat.capabilities
        .thinking_level_map
        .insert(ThinkingLevel::Low, None);
    chat.capabilities
        .thinking_level_map
        .insert(ThinkingLevel::High, Some("arbitrary".into()));
    let debug = format!("{chat:?}");
    assert!(!debug.contains("ENDPOINT_SENTINEL"));
    assert!(!debug.contains("HEADER_SENTINEL"));
    let mut models = Models::new(Arc::new(|| 73));
    models
        .register(chat.clone(), Arc::new(Adapter::default()))
        .unwrap();
    assert_eq!(models.find(&chat.identity), Some(chat.clone()));
    assert_ne!(
        chat.capabilities
            .thinking_level_map
            .get(&ThinkingLevel::Low),
        chat.capabilities
            .thinking_level_map
            .get(&ThinkingLevel::Medium)
    );
    chat.name.clear();
    let failure = models
        .register(chat, Arc::new(Adapter::default()))
        .unwrap_err();
    assert_eq!(failure.to_string(), "invalid local model catalog");
    assert!(!failure.to_string().contains("SENTINEL"));
}
