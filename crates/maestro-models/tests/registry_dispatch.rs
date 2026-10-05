mod support;
use maestro_models::*;
use std::sync::Arc;
use support::catalog::*;
use support::{
    auth::*,
    block_on,
    conformance::{Gate, WakeCounter, context, done, poll, steps},
};

#[test]
fn catalog_replacement_changes_next_lookup_and_dispatch() {
    let first = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let second = Arc::new(RecordingProvider::default());
    let per_model = Arc::new(RecordingProvider::default());
    let mut models = models();
    let base = entry("local", "base", "chat");
    let dropped = entry("local", "dropped", "chat");
    let separate = entry("other", "separate", "chat");
    models
        .register_catalog(
            "local",
            || Ok(vec![base.clone(), dropped.clone()]),
            first.clone(),
        )
        .unwrap();
    models
        .register(separate.clone(), per_model.clone())
        .unwrap();
    assert_eq!(
        block_on(models.complete(base.clone(), context(), local())).failure,
        None
    );
    let mut latest = base.clone();
    latest.endpoint = "new:endpoint".into();
    models
        .register_catalog("local", || Ok(vec![latest.clone()]), second.clone())
        .unwrap();
    assert_eq!(models.find(&base.identity), Some(latest.clone()));
    assert_eq!(models.find(&dropped.identity), None);
    assert_eq!(
        block_on(models.complete(dropped, context(), local())).failure,
        Some(Failure::UnknownModel)
    );
    assert_eq!(
        block_on(models.complete(base, context(), local())).failure,
        None
    );
    assert_eq!(first.calls().len(), 1);
    assert_eq!(second.calls.lock().unwrap().len(), 1);
    assert_eq!(second.calls.lock().unwrap()[0].0, latest);
    assert_eq!(
        block_on(models.complete(separate, context(), local())).failure,
        None
    );
    assert_eq!(per_model.calls.lock().unwrap().len(), 1);
}

#[test]
fn provider_removal_changes_next_lookup_and_dispatch() {
    let adapter = Arc::new(RecordingProvider::default());
    let mut models = models();
    let base = entry("local", "base", "chat");
    let native = entry("local", "base", "embedding");
    let other = entry("other", "base", "chat");
    for model in [&base, &native, &other] {
        models.register(model.clone(), adapter.clone()).unwrap();
    }
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: Some("override:endpoint".into()),
                models: vec![],
            },
        )
        .unwrap();
    assert!(models.remove_provider("local"));
    assert!(!models.remove_provider("local"));
    assert!(!models.remove_override("local"));
    assert_eq!(models.known(None), vec![other.clone()]);
    let resolver = Resolver::new(vec![]);
    assert_eq!(
        models.auth_status("local", &resolver),
        Err(Failure::UnknownProvider)
    );
    assert_eq!(
        resolver.statuses.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(
        block_on(models.complete(base.clone(), context(), local())).failure,
        Some(Failure::UnknownProvider)
    );
    assert_eq!(
        block_on(models.complete(other, context(), local())).failure,
        None
    );
    models.register(base.clone(), adapter.clone()).unwrap();
    assert_eq!(models.find(&base.identity), Some(base));
    // An empty catalog retains unmatched override state, which removal also clears.
    models
        .set_override("local", CatalogOverride::default())
        .unwrap();
    models
        .register_catalog("local", || Ok(vec![]), adapter)
        .unwrap();
    assert!(models.remove_provider("local"));
    assert!(!models.remove_provider("local"));
}

#[test]
fn in_flight_stream_retains_captured_adapter_and_metadata() {
    for mutation in [
        "replace",
        "remove-provider",
        "replace-override",
        "remove-override",
    ] {
        for phase in ["before-poll", "auth-wait", "body-wait"] {
            let auth_gate = Gate::default();
            let body_gate = Gate::default();
            let old = Arc::new(RecordingProvider {
                body_gate: (phase == "body-wait").then(|| body_gate.clone()),
                description: ProviderDescription {
                    headers: headers(&[("x-provider", "original-provider")]),
                    ..Default::default()
                },
                ..Default::default()
            });
            let next = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
            let mut models = models();
            let mut base = entry("local", "base", "chat");
            base.endpoint = "base:endpoint".into();
            base.headers = headers(&[("x-model", "base-header")]);
            let mut effective = base.clone();
            effective.endpoint = "captured:endpoint".into();
            effective.headers = headers(&[("x-model", "captured-header")]);
            effective.rates.input = 5.0;
            effective.rates_supplied = true;
            effective.input = vec!["captured-input".into()];
            effective.chat.as_mut().unwrap().context_window = Some(23);
            models.register(base.clone(), old.clone()).unwrap();
            models
                .set_override(
                    "local",
                    CatalogOverride {
                        endpoint: None,
                        models: vec![effective.clone()],
                    },
                )
                .unwrap();
            let resolver = Arc::new(Resolver {
                gate: (phase == "auth-wait").then(|| auth_gate.clone()),
                ..Resolver::new(vec![Ok(RequestAuth::ConfiguredWithoutSecret {
                    source: Some("captured-auth".into()),
                })])
            });
            let mut options = if phase != "body-wait" {
                resolving(resolver.clone())
            } else {
                local()
            };
            options.headers = headers(&[("x-request", "captured-request")]);
            let mut stream = models.stream(base.clone(), context(), options.clone());
            options
                .headers
                .insert("x-request".into(), "changed-after-call".into());
            let wake = Arc::new(WakeCounter::default());
            if phase == "auth-wait" {
                assert!(poll(Box::pin(stream.next()).as_mut(), &wake).is_pending());
                assert_eq!(resolver.count(), 1);
                assert!(old.calls.lock().unwrap().is_empty());
            } else if phase == "body-wait" {
                assert!(poll(Box::pin(stream.next()).as_mut(), &wake).is_pending());
                assert_eq!(old.calls.lock().unwrap().len(), 1);
            } else {
                assert!(old.calls.lock().unwrap().is_empty());
            }
            let mut changed = base.clone();
            changed.endpoint = "subsequent:endpoint".into();
            changed.rates.output = 7.0;
            changed.headers = headers(&[("x-model", "subsequent-header")]);
            changed.input = vec!["subsequent-input".into()];
            let expected_next = match mutation {
                "replace" => {
                    models
                        .register_catalog("local", || Ok(vec![changed]), next.clone())
                        .unwrap();
                    Some(effective.clone())
                }
                "remove-provider" => {
                    assert!(models.remove_provider("local"));
                    None
                }
                "replace-override" => {
                    models
                        .set_override(
                            "local",
                            CatalogOverride {
                                endpoint: None,
                                models: vec![changed.clone()],
                            },
                        )
                        .unwrap();
                    Some(changed)
                }
                _ => {
                    assert!(models.remove_override("local"));
                    Some(base.clone())
                }
            };
            assert_eq!(models.find(&base.identity), expected_next);
            auth_gate.release();
            body_gate.release();
            let mut terminal = None;
            while let Some(event) = block_on(stream.next()) {
                match event {
                    ModelEvent::Done { message, .. } => terminal = Some(message),
                    ModelEvent::Error { error, .. } => panic!("{mutation}/{phase}: {error:?}"),
                    _ => {}
                }
            }
            assert_eq!(terminal.unwrap().failure, None);
            {
                let calls = old.calls.lock().unwrap();
                assert_eq!(calls.len(), 1, "{mutation}/{phase}");
                assert_eq!(calls[0].0, effective, "{mutation}/{phase}");
                assert_eq!(
                    calls[0].1.headers,
                    headers(&[
                        ("x-provider", "original-provider"),
                        ("x-model", "captured-header"),
                        ("x-request", "captured-request")
                    ])
                );
            }
            let result = block_on(models.complete(base, context(), local()));
            if let Some(expected) = expected_next {
                assert_eq!(result.failure, None);
                if mutation == "replace" {
                    assert_eq!(next.calls().len(), 1);
                    assert_eq!(next.calls()[0].model, expected);
                } else {
                    let calls = old.calls.lock().unwrap();
                    assert_eq!(calls.len(), 2);
                    assert_eq!(calls[1].0, expected);
                }
            } else {
                assert_eq!(result.failure, Some(Failure::UnknownProvider));
            }
        }
    }
}

#[test]
fn dispatch_uses_effective_registration_not_stale_caller_metadata() {
    let adapter = Arc::new(RecordingProvider {
        description: ProviderDescription {
            headers: headers(&[("X-Shared", "provider"), ("x-provider", "provider")]),
            ..Default::default()
        },
        ..Default::default()
    });
    let mut models = models();
    let mut base = entry("local", "base", "chat");
    base.headers = headers(&[("x-shared", "base"), ("x-base", "base")]);
    models.register(base.clone(), adapter.clone()).unwrap();
    let mut effective = base.clone();
    effective.endpoint = "effective:endpoint".into();
    effective.rates.input = 6.0;
    effective.input = vec!["effective-capability".into()];
    effective.headers = headers(&[("x-shared", "effective"), ("x-model", "effective")]);
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: None,
                models: vec![effective.clone()],
            },
        )
        .unwrap();
    let mut stale = base.clone();
    stale.endpoint = "stale:endpoint".into();
    stale.rates.input = 999.0;
    stale.headers = headers(&[("x-stale", "stale")]);
    stale.input.clear();
    stale.chat = None;
    let resolver = Arc::new(Resolver::new(vec![Ok(secret(
        "REQUEST_SENTINEL",
        Some("selected"),
    ))]));
    let mut options = resolving(resolver.clone());
    options.headers = headers(&[("X-SHARED", "request")]);
    assert_eq!(
        block_on(models.complete(stale, context(), options)).failure,
        None
    );
    {
        let calls = adapter.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, effective);
        assert_eq!(
            calls[0].1.headers,
            headers(&[
                ("x-shared", "request"),
                ("x-provider", "provider"),
                ("x-model", "effective")
            ])
        );
        assert_auth(&calls[0].1.auth, Some("REQUEST_SENTINEL"), Some("selected"));
    }
    assert_eq!(resolver.count(), 1);
    effective.protocol = "new:protocol".into();
    models
        .set_override(
            "local",
            CatalogOverride {
                endpoint: None,
                models: vec![effective.clone()],
            },
        )
        .unwrap();
    assert_eq!(
        block_on(models.complete(base, context(), resolving(resolver.clone()))).failure,
        Some(Failure::UnsupportedOperation)
    );
    assert_eq!(resolver.count(), 1);
    assert_eq!(adapter.calls.lock().unwrap().len(), 1);
    assert_eq!(
        block_on(models.complete(
            models.find(&effective.identity).unwrap(),
            context(),
            local()
        ))
        .failure,
        None
    );
    assert_eq!(adapter.calls.lock().unwrap()[1].0, effective);
}

#[test]
fn advertised_operation_does_not_implement_dispatch() {
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let rejecting = Arc::new(RecordingProvider {
        supports_chat: false,
        ..Default::default()
    });
    let mut models = models();
    let fake_native = entry("scripted", "native", "native:other");
    let native = entry("recording", "native", "native:other");
    let chat = entry("recording", "chat", "chat");
    models.register(fake_native.clone(), fake.clone()).unwrap();
    models
        .register_catalog(
            "recording",
            || Ok(vec![native.clone(), chat.clone()]),
            rejecting.clone(),
        )
        .unwrap();
    assert_eq!(models.known(Some("chat")), vec![chat.clone()]);
    for request in [fake_native, native, chat.clone()] {
        assert_eq!(
            block_on(models.complete(request, context(), local())).failure,
            Some(Failure::UnsupportedOperation)
        );
    }
    let mut changed = chat.clone();
    changed.chat.as_mut().unwrap().reasoning = true;
    changed.input = vec!["arbitrary-capability".into()];
    models
        .set_override(
            "recording",
            CatalogOverride {
                endpoint: None,
                models: vec![changed],
            },
        )
        .unwrap();
    assert_eq!(
        block_on(models.complete(chat, context(), local())).failure,
        Some(Failure::UnsupportedOperation)
    );
    assert!(rejecting.calls.lock().unwrap().is_empty());
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
}
