mod support;
use maestro_credentials::*;
use maestro_models::*;
use std::sync::Arc;
use support::*;

#[test]
fn stored_key_reaches_only_selected_provider() {
    let credentials = owner(memory());
    credentials
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("SYNTHETIC_KEY"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    let chosen = scripted(2);
    let other = scripted(1);
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model("chosen"), chosen.clone()).unwrap();
    models.register(model("other"), other.clone()).unwrap();
    let options = StreamOptions {
        auth_resolver: Some(credentials),
        ..Default::default()
    };
    let mut stream = models.stream(model("chosen"), context(), options.clone());
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    assert!(matches!(events.last(), Some(ModelEvent::Done { .. })));
    assert_eq!(
        block_on(models.complete(model("chosen"), context(), options)).failure,
        None
    );
    assert_eq!(chosen.calls().len(), 2);
    for call in chosen.calls() {
        assert_secret(&call.options.auth, "SYNTHETIC_KEY", "stored");
    }
    assert!(other.calls().is_empty());
}

#[test]
fn runtime_override_precedes_stored_credentials() {
    let storage = CountingStorage::new(memory());
    let secrets = Secrets::new(&[], false);
    let mut inputs = options();
    inputs.secrets = secrets.clone();
    let credentials = Arc::new(Credentials::new(storage.clone(), inputs).unwrap());
    credentials
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("stored-key"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    let reads = storage.reads.load(std::sync::atomic::Ordering::SeqCst);
    credentials.set_runtime_auth("chosen".into(), Some(auth("runtime-key")));
    assert_secret(
        &block_on(credentials.resolve("chosen".into(), Cancellation::new())).unwrap(),
        "runtime-key",
        "runtime",
    );
    assert_eq!(
        storage.reads.load(std::sync::atomic::Ordering::SeqCst),
        reads
    );
    assert_eq!(
        secrets.resolves.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert!(!bytes(storage.as_ref()).contains("runtime-key"));
    credentials
        .set(
            "chosen",
            Credential::Refreshable(TokenExchangeResult {
                auth: auth("stored-token"),
                state: secret("opaque"),
                expires_at: Some(101),
            }),
            &Cancellation::new(),
        )
        .unwrap();
    let reads = storage.reads.load(std::sync::atomic::Ordering::SeqCst);
    assert_secret(
        &block_on(credentials.resolve("chosen".into(), Cancellation::new())).unwrap(),
        "runtime-key",
        "runtime",
    );
    assert_eq!(
        storage.reads.load(std::sync::atomic::Ordering::SeqCst),
        reads
    );
    credentials
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("stored-key"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    credentials.set_runtime_auth("chosen".into(), Some(auth("")));
    assert_eq!(
        block_on(credentials.resolve("chosen".into(), Cancellation::new())).unwrap_err(),
        Failure::MissingAuthentication
    );
    let fake = scripted(2);
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model("chosen"), fake.clone()).unwrap();
    for (value, expected) in [
        ("explicit", None),
        ("", Some(Failure::MissingAuthentication)),
    ] {
        let result = block_on(models.complete(
            model("chosen"),
            context(),
            StreamOptions {
                auth: Some(auth(value)),
                auth_resolver: Some(credentials.clone()),
                ..Default::default()
            },
        ));
        assert_eq!(result.failure, expected);
    }
    assert_eq!(fake.calls().len(), 1);
    credentials.set_runtime_auth("chosen".into(), None);
    assert_secret(
        &block_on(credentials.resolve("chosen".into(), Cancellation::new())).unwrap(),
        "stored-key",
        "stored",
    );
}

#[test]
fn refreshable_auth_requires_known_future_expiry() {
    for (expiry, available) in [
        (Some(101), true),
        (Some(100), false),
        (Some(99), false),
        (None, false),
    ] {
        for request_auth in [
            auth("token"),
            RequestAuth::ConfiguredWithoutSecret {
                source: Some("ignored".into()),
            },
        ] {
            let secrets = Secrets::new(&[("USABLE", "ambient")], false);
            let fallback = Fallback::new(Ok(auth("fallback")));
            let mut inputs = options();
            inputs.secrets = secrets.clone();
            inputs.fallback = Some(fallback.clone());
            inputs
                .environment_names
                .insert("chosen".into(), vec!["USABLE".into()]);
            let credentials = Credentials::new(memory(), inputs).unwrap();
            credentials
                .set(
                    "chosen",
                    Credential::Refreshable(TokenExchangeResult {
                        auth: request_auth.clone(),
                        state: secret("opaque-state"),
                        expires_at: expiry,
                    }),
                    &Cancellation::new(),
                )
                .unwrap();
            let result = block_on(credentials.resolve("chosen".into(), Cancellation::new()));
            if available {
                match result.unwrap() {
                    RequestAuth::Secret { secret, source } => {
                        assert_eq!(secret.expose(), "token");
                        assert_eq!(source.as_deref(), Some("stored"));
                    }
                    RequestAuth::ConfiguredWithoutSecret { source } => {
                        assert_eq!(source.as_deref(), Some("stored"))
                    }
                }
            } else {
                assert_eq!(result.unwrap_err(), Failure::MissingAuthentication);
            }
            assert!(secrets.names.lock().unwrap().is_empty());
            assert_eq!(
                secrets.resolves.load(std::sync::atomic::Ordering::SeqCst),
                0
            );
            assert!(fallback.calls.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn selected_stored_failure_never_uses_ambient_or_fallback() {
    use std::sync::atomic::Ordering;
    for (record, unresolved, fail_read, expected) in [
        (
            r#"{"type":"api_key","key":""}"#,
            false,
            false,
            Failure::MissingAuthentication,
        ),
        (
            r#"{"type":"api_key","key":"!secret-command"}"#,
            true,
            false,
            Failure::MissingAuthentication,
        ),
        (
            r#"{"type":"refreshable","auth":{"secret":""},"state":"state","expires_at":101}"#,
            false,
            false,
            Failure::MissingAuthentication,
        ),
        (
            r#"{"type":"unknown","key":"sentinel"}"#,
            false,
            false,
            Failure::AuthenticationFailed,
        ),
        (
            r#"{"type":"refreshable","auth":{"secret":"a","without_secret":true},"state":"state","expires_at":101}"#,
            false,
            false,
            Failure::AuthenticationFailed,
        ),
        (
            r#"{"type":"api_key","key":"key"}"#,
            false,
            true,
            Failure::AuthenticationFailed,
        ),
    ] {
        let initial = format!(r#"{{"chosen":{record},"unrelated":{{"alien":[1,true]}}}}"#);
        let storage = CountingStorage::new(Arc::new(MemoryCredentialStorage::new(Some(secret(
            &initial,
        )))));
        let secrets = Secrets::new(&[("NAMED", "ambient")], unresolved);
        let fallback = Fallback::new(Ok(auth("fallback")));
        let mut inputs = options();
        inputs
            .environment_names
            .insert("chosen".into(), vec!["NAMED".into()]);
        inputs.secrets = secrets.clone();
        inputs.fallback = Some(fallback.clone());
        let credentials = Arc::new(Credentials::new(storage.clone(), inputs).unwrap());
        storage.fail.store(fail_read, Ordering::SeqCst);
        let (result, adapter) = request(credentials, "chosen");
        assert_eq!(result.failure, Some(expected));
        assert!(adapter.calls().is_empty());
        assert!(secrets.names.lock().unwrap().is_empty());
        assert!(fallback.calls.lock().unwrap().is_empty());
    }
    let storage = Arc::new(MemoryCredentialStorage::new(Some(secret(
        r#"{"chosen":{"type":"api_key","key":"valid"},"unrelated":{"alien":true}}"#,
    ))));
    assert_eq!(request(owner(storage), "chosen").0.failure, None);
}

#[test]
fn registered_environment_precedes_configured_fallback() {
    for values in [
        vec![("second", "ENV")],
        vec![("first", ""), ("second", "ENV")],
        vec![],
    ] {
        let secrets = Secrets::new(&values, false);
        let fallback = Fallback::new(Ok(auth("FALLBACK")));
        let mut inputs = options();
        inputs
            .environment_names
            .insert("arbitrary/id".into(), vec!["first".into(), "second".into()]);
        inputs.secrets = secrets.clone();
        inputs.fallback = Some(fallback.clone());
        let credentials = Credentials::new(memory(), inputs).unwrap();
        let signal = Cancellation::new();
        let resolved =
            block_on(credentials.resolve("arbitrary/id".into(), signal.clone())).unwrap();
        if values.is_empty() {
            assert_secret(&resolved, "FALLBACK", "fallback");
            let calls = fallback.calls.lock().unwrap();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "arbitrary/id");
            calls[0].1.cancel();
            assert!(signal.is_cancelled());
        } else {
            assert_secret(&resolved, "ENV", "environment");
            assert!(fallback.calls.lock().unwrap().is_empty());
        }
        assert_eq!(*secrets.names.lock().unwrap(), vec!["first", "second"]);
    }
    assert_eq!(
        block_on(owner(memory()).resolve("absent".into(), Cancellation::new())).unwrap_err(),
        Failure::MissingAuthentication
    );
    for (result, expected) in [
        (Err(Failure::Transport), Failure::AuthenticationFailed),
        (
            Err(Failure::MissingAuthentication),
            Failure::MissingAuthentication,
        ),
        (Ok(auth("")), Failure::MissingAuthentication),
    ] {
        let mut inputs = options();
        inputs.fallback = Some(Fallback::new(result));
        assert_eq!(
            block_on(
                Credentials::new(memory(), inputs)
                    .unwrap()
                    .resolve("x".into(), Cancellation::new())
            )
            .unwrap_err(),
            expected
        );
    }
}

#[test]
fn metadata_inspection_has_no_secret_effects() {
    use std::sync::atomic::Ordering;
    let storage = CountingStorage::new(Arc::new(MemoryCredentialStorage::new(Some(secret(
        r#"{"stored":{"type":"api_key","key":"!COMMAND"},"empty":{"type":"api_key","key":""}}"#,
    )))));
    let secrets = Secrets::new(&[("NAME", "VALUE")], false);
    let fallback = Fallback::new(Ok(auth("FALLBACK")));
    let mut inputs = options();
    inputs.secrets = secrets.clone();
    inputs.fallback = Some(fallback.clone());
    inputs
        .environment_names
        .insert("environment".into(), vec!["NAME".into()]);
    let credentials = Arc::new(Credentials::new(storage.clone(), inputs).unwrap());
    credentials.set_runtime_auth("stored".into(), Some(auth("runtime")));
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model("empty"), scripted(1)).unwrap();
    for _ in 0..3 {
        assert_eq!(credentials.list(), vec!["empty", "stored"]);
        for (provider, configured, source) in [
            ("stored", true, "runtime"),
            ("empty", true, "stored"),
            ("environment", false, "environment"),
            ("other", true, "fallback"),
        ] {
            assert_eq!(
                credentials.status(provider),
                AuthStatus {
                    configured,
                    source: Some(source.into())
                }
            );
        }
        assert_eq!(
            models
                .auth_status("empty", credentials.as_ref())
                .unwrap()
                .source
                .as_deref(),
            Some("stored")
        );
    }
    assert_eq!(storage.reads.load(Ordering::SeqCst), 1);
    assert_eq!(secrets.resolves.load(Ordering::SeqCst), 0);
    assert!(secrets.names.lock().unwrap().is_empty());
    assert!(fallback.calls.lock().unwrap().is_empty());
    let no_source = owner(memory());
    assert_eq!(
        no_source.status("x"),
        AuthStatus {
            configured: false,
            source: None
        }
    );
}

#[test]
fn secret_sentinels_stay_out_of_public_observations() {
    use std::error::Error;
    let sentinels = [
        "KEY_SENTINEL",
        "TOKEN_SENTINEL",
        "STATE_SENTINEL",
        "COMMAND_SENTINEL",
        "OUTPUT_SENTINEL",
        "STDERR_SENTINEL",
        "MALFORMED_SENTINEL",
    ];
    for (value, unresolved) in [
        (sentinels[0], false),
        ("!COMMAND_SENTINEL", true),
        (sentinels[4], false),
    ] {
        let mut inputs = options();
        inputs.secrets = Secrets::new(&[], unresolved);
        let credentials = Arc::new(Credentials::new(memory(), inputs).unwrap());
        let credential = Credential::ApiKey {
            value: secret(value),
        };
        let debug = format!("{credential:?}");
        credentials
            .set("chosen", credential, &Cancellation::new())
            .unwrap();
        let original = context();
        let before = format!("{original:?}");
        let fake = scripted(1);
        let mut models = Models::new(Arc::new(|| 123));
        models.register(model("chosen"), fake).unwrap();
        let mut stream = models.stream(
            model("chosen"),
            original.clone(),
            StreamOptions {
                auth_resolver: Some(credentials.clone()),
                ..Default::default()
            },
        );
        let mut events = vec![];
        while let Some(event) = block_on(stream.next()) {
            events.push(event);
        }
        let observation = format!(
            "{debug} {:?} {:?} {events:?} {before}",
            credentials.list(),
            credentials.status("chosen")
        );
        for sentinel in sentinels {
            assert!(!observation.contains(sentinel));
        }
        assert_eq!(original, context());
    }
    let token = Credential::Refreshable(TokenExchangeResult {
        auth: auth(sentinels[1]),
        state: secret(sentinels[2]),
        expires_at: Some(101),
    });
    for sentinel in sentinels {
        assert!(!format!("{token:?}").contains(sentinel));
    }
    for error in [
        CredentialError::Cancelled,
        CredentialError::ReadOnly,
        CredentialError::Contended,
        CredentialError::Malformed,
        CredentialError::Storage,
        CredentialError::InvalidPath,
    ] {
        assert!(error.source().is_none());
        for sentinel in sentinels {
            assert!(!format!("{error} {error:?}").contains(sentinel));
        }
    }
    let malformed = Credentials::new(
        Arc::new(MemoryCredentialStorage::new(Some(secret(
            "MALFORMED_SENTINEL",
        )))),
        options(),
    )
    .err()
    .unwrap();
    assert_eq!(malformed, CredentialError::Malformed);
}

#[test]
fn stored_token_metadata_and_observations_exclude_access_and_refresh_secrets() {
    let sentinels = ["ACCESS_TOKEN_SENTINEL", "REFRESH_TOKEN_SENTINEL"];
    let credentials = owner(memory());
    credentials
        .set(
            "chosen",
            Credential::Refreshable(TokenExchangeResult {
                auth: auth(sentinels[0]),
                state: secret(sentinels[1]),
                expires_at: Some(101),
            }),
            &Cancellation::new(),
        )
        .unwrap();
    assert_eq!(credentials.list(), vec!["chosen"]);
    assert_eq!(
        credentials.status("chosen"),
        AuthStatus {
            configured: true,
            source: Some("stored".into()),
        }
    );
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model("chosen"), scripted(1)).unwrap();
    let original = context();
    let before = format!("{original:?}");
    let mut stream = models.stream(
        model("chosen"),
        original.clone(),
        StreamOptions {
            auth_resolver: Some(credentials.clone()),
            ..Default::default()
        },
    );
    let mut events = vec![];
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    assert!(matches!(events.last(), Some(ModelEvent::Done { .. })));
    let metadata = format!(
        "{:?} {:?} {:?}",
        credentials.list(),
        credentials.status("chosen"),
        models.auth_status("chosen", credentials.as_ref()).unwrap(),
    );
    for observation in [
        metadata,
        format!("{events:?}"),
        before,
        format!("{original:?}"),
    ] {
        for sentinel in sentinels {
            assert!(!observation.contains(sentinel));
        }
    }
    assert_eq!(original, context());
}

#[test]
fn cancelled_resolution_never_dispatches() {
    for fallback in [false, true] {
        for pre_cancelled in [false, true] {
            for readiness_tie in [false, true] {
                let (gate, entered) = gate();
                let storage = memory();
                let mut inputs = options();
                if fallback {
                    inputs.fallback = Some(gate.clone());
                } else {
                    inputs.secrets = gate.clone();
                }
                let credentials = Arc::new(Credentials::new(storage, inputs).unwrap());
                if !fallback {
                    credentials
                        .set(
                            "chosen",
                            Credential::ApiKey {
                                value: secret("!unused"),
                            },
                            &Cancellation::new(),
                        )
                        .unwrap();
                }
                let fake = scripted(1);
                let selected = fake.clone();
                let mut models = Models::new(Arc::new(|| 123));
                models.register(model("chosen"), fake).unwrap();
                let signal = Cancellation::new();
                if pre_cancelled {
                    signal.cancel();
                }
                let worker_signal = signal.clone();
                let worker = std::thread::spawn(move || {
                    let mut stream = models.stream(
                        model("chosen"),
                        context(),
                        StreamOptions {
                            cancellation: worker_signal,
                            auth_resolver: Some(credentials),
                            ..Default::default()
                        },
                    );
                    let mut events = vec![];
                    while let Some(event) = block_on(stream.next()) {
                        events.push(event);
                    }
                    events
                });
                if !pre_cancelled {
                    entered.recv().unwrap();
                    signal.cancel();
                    if readiness_tie {
                        gate.release.cancel();
                    }
                }
                let events = worker.join().unwrap();
                assert_eq!(events.len(), 1);
                assert!(
                    matches!(&events[0], ModelEvent::Error { reason: StopReason::Aborted, error } if error.failure == Some(Failure::Cancelled))
                );
                assert!(selected.calls().is_empty());
            }
        }
    }
    let (gate, entered) = gate();
    let mut inputs = options();
    inputs.secrets = gate.clone();
    let credentials = Arc::new(Credentials::new(memory(), inputs).unwrap());
    credentials
        .set(
            "chosen",
            Credential::ApiKey {
                value: secret("pending"),
            },
            &Cancellation::new(),
        )
        .unwrap();
    let fake = scripted(1);
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model("chosen"), fake.clone()).unwrap();
    let signal = Cancellation::new();
    let mut stream = models.stream(
        model("chosen"),
        context(),
        StreamOptions {
            auth_resolver: Some(credentials),
            cancellation: signal.clone(),
            ..Default::default()
        },
    );
    {
        let mut read = std::pin::pin!(stream.next());
        loop {
            assert!(poll_once(read.as_mut()).is_pending());
            if entered.try_recv().is_ok() {
                break;
            }
            std::thread::park();
        }
    }
    {
        let mut read = std::pin::pin!(stream.next());
        assert!(poll_once(read.as_mut()).is_pending());
        assert_eq!(gate.count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    signal.cancel();
    assert!(matches!(
        block_on(stream.next()),
        Some(ModelEvent::Error {
            reason: StopReason::Aborted,
            ..
        })
    ));
    assert!(block_on(stream.next()).is_none());
    assert!(fake.calls().is_empty());
    drop(stream);
    gate.release.cancel();
}
