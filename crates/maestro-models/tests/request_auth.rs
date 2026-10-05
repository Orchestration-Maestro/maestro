mod support;
use maestro_models::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use support::conformance::*;

struct NeverResolve(AtomicUsize);
impl AuthResolver for NeverResolve {
    fn status(&self, _: &str) -> AuthStatus {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("explicit auth called status")
    }
    fn resolve(
        &self,
        _: String,
        _: Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        panic!("explicit auth called resolve")
    }
}
#[test]
fn explicit_auth_bypasses_resolver() {
    let resolver = Arc::new(NeverResolve(AtomicUsize::new(0)));
    for (auth, failure) in [
        (
            RequestAuth::Secret {
                secret: SecretString::new("explicit-token".into()),
                source: Some("request".into()),
            },
            None,
        ),
        (RequestAuth::ConfiguredWithoutSecret { source: None }, None),
        (
            RequestAuth::Secret {
                secret: SecretString::new(String::new()),
                source: None,
            },
            Some(Failure::MissingAuthentication),
        ),
    ] {
        let (models, fake) = fixture(vec![done()]);
        let events = collect(
            models.stream(
                model(),
                context(),
                StreamOptions {
                    auth: Some(auth),
                    auth_resolver: Some(resolver.clone()),
                    ..Default::default()
                },
            ),
            &model(),
            73,
        );
        assert_eq!(terminal(&events).failure, failure);
        assert_eq!(fake.calls().len(), usize::from(failure.is_none()));
        if failure.is_some() {
            assert_eq!(trace(&events), vec![("error", None)]);
            assert_eq!(fake.pending(), 1);
        }
    }
    assert_eq!(resolver.0.load(Ordering::SeqCst), 0);
}

#[test]
fn resolver_receives_only_selected_provider_and_is_request_scoped() {
    use support::auth::*;
    let selected = Arc::new(ScriptedProvider::new(vec![
        steps(vec![done()]),
        steps(vec![done()]),
    ]));
    let other = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let mut models = registry(selected.clone());
    let mut other_model = model();
    other_model.identity.provider = "other".into();
    models.register(other_model, other.clone()).unwrap();
    let resolver = Arc::new(Resolver::new(vec![
        Ok(secret("first", Some("one"))),
        Ok(secret("second", Some("two"))),
    ]));
    for (value, label) in [("first", "one"), ("second", "two")] {
        let options = resolving(resolver.clone());
        let events = collect(
            models.stream(model(), context(), options.clone()),
            &model(),
            73,
        );
        assert_eq!(terminal(&events).stop_reason, Some(StopReason::Stop));
        let calls = selected.calls();
        assert_auth(
            &calls.last().unwrap().options.auth,
            Some(value),
            Some(label),
        );
        let resolved = resolver.calls.lock().unwrap();
        assert_eq!(resolved.last().unwrap().0, "local");
        resolved.last().unwrap().1.cancel();
        assert!(options.cancellation.is_cancelled());
    }
    assert_eq!(resolver.count(), 2);
    assert_eq!(resolver.statuses.load(Ordering::SeqCst), 0);
    assert!(other.calls().is_empty());
    assert_eq!(other.pending(), 1);
}

#[test]
fn configured_without_secret_is_explicit_and_preserves_source() {
    use support::auth::*;
    for label in [None, Some("local-service")] {
        for resolved in [false, true] {
            let auth = RequestAuth::ConfiguredWithoutSecret {
                source: label.map(str::to_owned),
            };
            let resolver = Arc::new(Resolver::new(vec![Ok(auth.clone())]));
            let options = if resolved {
                resolving(resolver.clone())
            } else {
                StreamOptions {
                    auth: Some(auth),
                    ..Default::default()
                }
            };
            let (models, fake) = fixture(vec![done()]);
            let result = support::block_on(models.complete(model(), context(), options));
            assert_eq!(result.stop_reason, Some(StopReason::Stop));
            assert_auth(&fake.calls()[0].options.auth, None, label);
            assert_eq!(resolver.count(), usize::from(resolved));
        }
    }
    let (models, fake) = fixture(vec![done()]);
    let events = collect(
        models.stream(model(), context(), StreamOptions::default()),
        &model(),
        73,
    );
    assert_eq!(
        terminal(&events).failure,
        Some(Failure::MissingAuthentication)
    );
    assert!(fake.calls().is_empty());
}

#[test]
fn missing_and_failed_resolution_are_safe_pre_dispatch_errors() {
    use support::auth::*;
    struct DescribedScript(Arc<ScriptedProvider>);
    impl Provider for DescribedScript {
        fn supports(&self, operation: &str) -> bool {
            self.0.supports(operation)
        }
        fn description(&self) -> ProviderDescription {
            ProviderDescription {
                ambient_credential_names: vec!["MAESTRO_SYNTHETIC_AMBIENT_CREDENTIAL".into()],
                ..Default::default()
            }
        }
        fn stream(
            &self,
            model: Model,
            context: Context,
            options: ProviderOptions,
        ) -> Result<Box<dyn ProviderStream>, Failure> {
            self.0.stream(model, context, options)
        }
    }
    for (result, expected, text) in [
        (
            None,
            Failure::MissingAuthentication,
            "missing request authentication",
        ),
        (
            Some(Ok(secret("", None))),
            Failure::MissingAuthentication,
            "missing request authentication",
        ),
        (
            Some(Err(Failure::MissingAuthentication)),
            Failure::MissingAuthentication,
            "missing request authentication",
        ),
        (
            Some(Err(Failure::AuthenticationFailed)),
            Failure::AuthenticationFailed,
            "request authentication failed",
        ),
        (
            Some(Err(Failure::AuthenticationFailed)),
            Failure::AuthenticationFailed,
            "request authentication failed",
        ),
        (
            Some(Err(Failure::Transport)),
            Failure::AuthenticationFailed,
            "request authentication failed",
        ),
    ] {
        let selected = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
        let other = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
        let mut models = registry(Arc::new(DescribedScript(selected.clone())));
        let mut other_model = model();
        other_model.identity.provider = "usable-other".into();
        models.register(other_model, other.clone()).unwrap();
        let options = result
            .map(|r| resolving(Arc::new(Resolver::new(vec![r]))))
            .unwrap_or_default();
        let events = collect(models.stream(model(), context(), options), &model(), 73);
        assert_eq!(trace(&events), vec![("error", None)]);
        assert_eq!(terminal(&events).failure, Some(expected));
        assert_eq!(expected.to_string(), text);
        assert!(selected.calls().is_empty());
        assert!(other.calls().is_empty());
        assert_eq!(selected.pending(), 1);
        assert_eq!(other.pending(), 1);
    }
}

#[test]
fn identity_and_capability_fail_before_auth_resolution() {
    use support::auth::*;
    let resolver = Arc::new(Resolver::new(vec![]));
    let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let models = registry(fake.clone());
    let mut unknown_provider = model();
    unknown_provider.identity.provider = "missing".into();
    let mut unknown_model = model();
    unknown_model.identity.model = "missing".into();
    let mut protocol = model();
    protocol.protocol = "missing".into();
    let mut operation = model();
    operation.identity.operation = "embedding".into();
    for (request, expected) in [
        (unknown_provider, Failure::UnknownProvider),
        (unknown_model, Failure::UnknownModel),
        (protocol, Failure::UnsupportedOperation),
        (operation, Failure::UnsupportedOperation),
    ] {
        let events = collect(
            models.stream(request.clone(), context(), resolving(resolver.clone())),
            &request,
            73,
        );
        assert_eq!(trace(&events), vec![("error", None)]);
        assert_eq!(terminal(&events).failure, Some(expected));
    }
    let mut direct = DirectProvider::new(vec![done()]);
    direct.supports_chat = false;
    let direct = Arc::new(direct);
    let models = registry(direct.clone());
    let result =
        support::block_on(models.complete(model(), context(), resolving(resolver.clone())));
    assert_eq!(result.failure, Some(Failure::UnsupportedOperation));
    assert_eq!(direct.calls.load(Ordering::SeqCst), 0);
    assert_eq!(resolver.count(), 0);
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
}

#[test]
fn credentials_reach_only_selected_adapter() {
    use support::auth::*;
    let first = Arc::new(Adapter::default());
    let second = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
    let mut models = registry(first.clone());
    let mut second_model = model();
    second_model.identity.provider = "other".into();
    models
        .register(second_model.clone(), second.clone())
        .unwrap();
    let resolver = Arc::new(Resolver::new(vec![Ok(secret(
        "ONLY_FIRST",
        Some("resolved"),
    ))]));
    let mut options = resolving(resolver.clone());
    options
        .headers
        .insert("x-credential".into(), "FIRST_HEADER".into());
    let result = support::block_on(models.complete(model(), context(), options));
    assert_eq!(result.failure, None);
    let calls = first.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_auth(&calls[0].2.auth, Some("ONLY_FIRST"), Some("resolved"));
    assert_eq!(calls[0].2.headers["x-credential"], "FIRST_HEADER");
    drop(calls);
    assert!(second.calls().is_empty());
    assert_eq!(second.pending(), 1);
    let mut options = local();
    options.auth = Some(secret("ONLY_SECOND", Some("explicit")));
    options.auth_resolver = Some(resolver.clone());
    options
        .headers
        .insert("x-credential".into(), "SECOND_HEADER".into());
    let result = support::block_on(models.complete(second_model, context(), options));
    assert_eq!(result.failure, None);
    assert_auth(
        &second.calls()[0].options.auth,
        Some("ONLY_SECOND"),
        Some("explicit"),
    );
    assert_eq!(
        second.calls()[0].options.headers["x-credential"],
        "SECOND_HEADER"
    );
    assert_eq!(first.calls.lock().unwrap().len(), 1);
    assert_eq!(resolver.count(), 1);
}

#[test]
fn headers_overlay_case_insensitively_without_mutating_inputs() {
    use support::auth::*;
    let description = ProviderDescription {
        headers: headers(&[("X-Shared", "provider"), ("X-Provider", "literal-$HOME")]),
        ..Default::default()
    };
    let adapter = Arc::new(Adapter {
        description: description.clone(),
        ..Default::default()
    });
    let mut registered = model();
    registered.headers = headers(&[("x-shared", "model"), ("X-Model", "model-only")]);
    let mut models = Models::new(Arc::new(|| 73));
    models
        .register(registered.clone(), adapter.clone())
        .unwrap();
    let mut options = local();
    options.headers = headers(&[("X-SHARED", "request"), ("X-Empty", ""), ("x-tab", "a\tb")]);
    let before = options.headers.clone();
    let mut requested = registered.clone();
    requested.headers = headers(&[("ignored", "caller-model")]);
    let result = support::block_on(models.complete(requested.clone(), context(), options.clone()));
    assert_eq!(result.failure, None);
    let calls = adapter.calls.lock().unwrap();
    assert_eq!(
        calls[0].2.headers,
        headers(&[
            ("x-shared", "request"),
            ("x-provider", "literal-$HOME"),
            ("x-model", "model-only"),
            ("x-empty", ""),
            ("x-tab", "a\tb")
        ])
    );
    drop(calls);
    assert_eq!(options.headers, before);
    assert_eq!(registered.headers["x-shared"], "model");
    assert_eq!(requested.headers["ignored"], "caller-model");
    assert_eq!(adapter.description, description);
    for bad in [
        headers(&[("X", "one"), ("x", "two")]),
        headers(&[("", "bad")]),
        headers(&[("bad name", "bad")]),
        headers(&[("nonascii-é", "bad")]),
        headers(&[("x", "SENTINEL\r\n")]),
        headers(&[("x", "\0")]),
        headers(&[("x", "\u{7f}")]),
    ] {
        for layer in 0..3 {
            let mut adapter = Adapter::default();
            let mut registered = model();
            let mut options = StreamOptions::default();
            match layer {
                0 => adapter.description.headers = bad.clone(),
                1 => registered.headers = bad.clone(),
                _ => options.headers = bad.clone(),
            }
            let adapter = Arc::new(adapter);
            let mut models = Models::new(Arc::new(|| 73));
            models
                .register(registered.clone(), adapter.clone())
                .unwrap();
            let resolver = Arc::new(Resolver::new(vec![]));
            options.auth_resolver = Some(resolver.clone());
            let events = collect(
                models.stream(registered.clone(), context(), options),
                &registered,
                73,
            );
            assert_eq!(
                terminal(&events).failure,
                Some(Failure::InvalidRequestHeaders)
            );
            assert_eq!(trace(&events), vec![("error", None)]);
            assert_eq!(resolver.count(), 0);
            assert!(adapter.calls.lock().unwrap().is_empty());
            assert!(!format!("{:?}", events).contains("SENTINEL"));
        }
    }
}

#[test]
fn secret_bearing_values_do_not_enter_public_observations() {
    use support::auth::*;
    const SENTINELS: [&str; 5] = [
        "AUTH_SENTINEL",
        "PROVIDER_SENTINEL",
        "MODEL_SENTINEL",
        "REQUEST_SENTINEL",
        "STATE_SENTINEL",
    ];
    let description = ProviderDescription {
        headers: headers(&[("unknown", SENTINELS[1])]),
        ..Default::default()
    };
    let mut registered = model();
    registered.headers = headers(&[("x-model", SENTINELS[2])]);
    let auth = secret(SENTINELS[0], Some("safe-label"));
    let mut options = StreamOptions {
        auth: Some(auth.clone()),
        ..Default::default()
    };
    options.headers = headers(&[("x-request", SENTINELS[3])]);
    let provider_options = ProviderOptions {
        cancellation: options.cancellation.clone(),
        auth: auth.clone(),
        headers: options.headers.clone(),
    };
    let state = SecretString::new(SENTINELS[4].into());
    let exchange_result = TokenExchangeResult {
        auth: auth.clone(),
        state: state.clone(),
        expires_at: None,
    };
    let formatted = format!(
        "{auth:?} {options:?} {provider_options:?} {description:?} {registered:?} {state:?} {exchange_result:?}"
    );
    for sentinel in SENTINELS {
        assert!(!formatted.contains(sentinel), "unsafe Debug");
    }
    let original = context();
    let transcript = format!("{original:?}");
    for resolved in [false, true] {
        for outcome in 0..4 {
            let adapter = Arc::new(Adapter {
                description: description.clone(),
                ..Default::default()
            });
            let mut models = Models::new(Arc::new(|| 73));
            models
                .register(registered.clone(), adapter.clone())
                .unwrap();
            let resolver = Arc::new(Resolver::new(vec![if outcome == 2 {
                Err(Failure::AuthenticationFailed)
            } else {
                Ok(auth.clone())
            }]));
            let mut request = options.clone();
            request.cancellation = Cancellation::new();
            if resolved || outcome == 2 {
                request.auth = None;
                request.auth_resolver = Some(resolver);
            }
            if outcome == 1 {
                request
                    .headers
                    .insert("invalid name".into(), SENTINELS[3].into());
            }
            if outcome == 3 {
                request.cancellation.cancel();
            }
            let events = collect(
                models.stream(registered.clone(), original.clone(), request),
                &registered,
                73,
            );
            let observed = format!("{events:?} {:?}", terminal(&events));
            for sentinel in SENTINELS {
                assert!(!observed.contains(sentinel));
                assert!(!transcript.contains(sentinel));
            }
            for failure in [
                Failure::MissingAuthentication,
                Failure::AuthenticationFailed,
                Failure::InvalidRequestHeaders,
                Failure::Cancelled,
            ] {
                for sentinel in SENTINELS {
                    assert!(!format!("{failure} {failure:?}").contains(sentinel));
                }
            }
        }
    }
    assert_eq!(original, context());
    if std::env::var_os("MAESTRO_AUTH_DEBUG_CHILD").is_some() {
        println!("{formatted}");
        eprintln!("{formatted}");
    } else {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "secret_bearing_values_do_not_enter_public_observations",
                "--nocapture",
            ])
            .env("MAESTRO_AUTH_DEBUG_CHILD", "1")
            .output()
            .unwrap();
        assert!(output.status.success());
        for sentinel in SENTINELS {
            assert!(!String::from_utf8_lossy(&output.stdout).contains(sentinel));
            assert!(!String::from_utf8_lossy(&output.stderr).contains(sentinel));
        }
    }
}

#[test]
fn resolver_and_adapter_swaps_preserve_caller_and_completion() {
    use support::auth::*;
    struct LocalResolver;
    impl AuthResolver for LocalResolver {
        fn status(&self, _: &str) -> AuthStatus {
            AuthStatus {
                configured: true,
                source: None,
            }
        }
        fn resolve(
            &self,
            _: String,
            _: Cancellation,
        ) -> Pin<Box<dyn Future<Output = Result<RequestAuth, Failure>> + Send + '_>> {
            Box::pin(async { Ok(RequestAuth::ConfiguredWithoutSecret { source: None }) })
        }
    }
    fn caller(provider: Arc<dyn Provider>, resolver: Arc<dyn AuthResolver>) -> AssistantMessage {
        let models = registry(provider);
        let events = collect(
            models.stream(model(), context(), resolving(resolver.clone())),
            &model(),
            73,
        );
        let completed = support::block_on(models.complete(model(), context(), resolving(resolver)));
        assert_eq!(*terminal(&events), completed);
        assert_eq!(completed.timestamp, 73);
        completed
    }
    let counted = Arc::new(Resolver::new(vec![
        Ok(secret("one", None)),
        Ok(secret("two", None)),
    ]));
    let scripted = Arc::new(ScriptedProvider::new(vec![
        steps(vec![done()]),
        steps(vec![done()]),
    ]));
    let first = caller(scripted.clone(), counted.clone());
    let independent = Arc::new(Adapter::default());
    let second = caller(independent.clone(), Arc::new(LocalResolver));
    assert_eq!(first, second);
    assert_eq!(counted.count(), 2);
    assert_eq!(scripted.calls().len(), 2);
    assert_eq!(independent.calls.lock().unwrap().len(), 2);
    let adapter = Arc::new(Adapter::default());
    let models = registry(adapter.clone());
    assert_eq!(
        support::block_on(models.complete(model(), context(), local())),
        first
    );
    assert_eq!(adapter.calls.lock().unwrap().len(), 1);
}
