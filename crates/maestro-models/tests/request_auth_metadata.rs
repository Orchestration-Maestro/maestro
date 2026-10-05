mod support;
use maestro_models::*;
use std::sync::{Arc, atomic::Ordering};
use support::{auth::*, block_on, conformance::*};

#[test]
fn auth_status_is_metadata_only_and_does_not_predict_request_success() {
    let (models, fake) = fixture(vec![done()]);
    for configured in [false, true] {
        for label in [None, Some("declared")] {
            let mut resolver = Resolver::new(vec![Err(Failure::AuthenticationFailed)]);
            resolver.status = AuthStatus {
                configured,
                source: label.map(str::to_owned),
            };
            resolver.gate = Some(Gate::default());
            assert_eq!(
                models.auth_status("unknown", &resolver),
                Err(Failure::UnknownProvider)
            );
            assert_eq!(resolver.statuses.load(Ordering::SeqCst), 0);
            assert_eq!(
                models.auth_status("local", &resolver),
                Ok(resolver.status.clone())
            );
            assert_eq!(resolver.statuses.load(Ordering::SeqCst), 1);
            assert_eq!(resolver.count(), 0);
        }
    }
    let resolver = Arc::new(Resolver::new(vec![Err(Failure::AuthenticationFailed)]));
    assert!(
        models
            .auth_status("local", resolver.as_ref())
            .unwrap()
            .configured
    );
    let result = block_on(models.complete(model(), context(), resolving(resolver.clone())));
    assert_eq!(result.failure, Some(Failure::AuthenticationFailed));
    assert_eq!(resolver.count(), 1);
    assert!(fake.calls().is_empty());
    assert_eq!(fake.pending(), 1);
}

#[test]
fn ambient_names_are_inert_provider_description_data() {
    let name = "MAESTRO_SYNTHETIC_AMBIENT_CREDENTIAL";
    let description = ProviderDescription {
        ambient_credential_names: vec![name.into(), "custom/name".into()],
        ..Default::default()
    };
    let adapter = Arc::new(Adapter {
        description: description.clone(),
        ..Default::default()
    });
    assert_eq!(adapter.description(), description);
    assert_eq!(
        ScriptedProvider::new(vec![]).description(),
        ProviderDescription::default()
    );
    let models = registry(adapter.clone());
    for options in [
        StreamOptions::default(),
        resolving(Arc::new(Resolver::new(vec![Err(
            Failure::AuthenticationFailed,
        )]))),
    ] {
        let result = block_on(models.complete(model(), context(), options));
        assert!(matches!(
            result.failure,
            Some(Failure::MissingAuthentication | Failure::AuthenticationFailed)
        ));
    }
    assert!(adapter.calls.lock().unwrap().is_empty());
    if std::env::var_os("MAESTRO_AMBIENT_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "ambient_names_are_inert_provider_description_data",
            ])
            .env("MAESTRO_AMBIENT_CHILD", "1")
            .env(name, "AMBIENT_SENTINEL")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert_eq!(std::env::var(name).unwrap(), "AMBIENT_SENTINEL");
    }
}

#[test]
fn optional_exchange_is_owner_invoked_and_defaults_to_none() {
    assert!(ScriptedProvider::new(vec![]).token_exchange().is_none());
    let exchange = Arc::new(Exchange {
        calls: Default::default(),
        gate: None,
        failure: false,
    });
    let adapter = Arc::new(Adapter {
        exchange: Some(exchange.clone()),
        ..Default::default()
    });
    let handle = adapter.token_exchange().unwrap();
    let models = registry(adapter.clone());
    let resolver = Resolver::new(vec![]);
    assert!(models.auth_status("local", &resolver).unwrap().configured);
    assert_eq!(
        block_on(models.complete(model(), context(), local())).failure,
        None
    );
    assert_eq!(exchange.calls.load(Ordering::SeqCst), 0);
    let result = block_on(handle.exchange(
        SecretString::new("STATE_SENTINEL".into()),
        Cancellation::new(),
    ))
    .unwrap();
    assert_auth(&result.auth, Some("ROTATED_AUTH"), Some("exchange"));
    assert_eq!(result.state.expose(), "ROTATED_STATE");
    assert_eq!(result.expires_at, Some(123));
    for sentinel in ["ROTATED_AUTH", "ROTATED_STATE"] {
        assert!(!format!("{result:?}").contains(sentinel));
    }
    assert_eq!(exchange.calls.load(Ordering::SeqCst), 1);
    let failed = Exchange {
        calls: Default::default(),
        gate: None,
        failure: true,
    };
    assert!(matches!(
        block_on(failed.exchange(
            SecretString::new("STATE_SENTINEL".into()),
            Cancellation::new()
        )),
        Err(Failure::AuthenticationFailed)
    ));
    let gate = Gate::default();
    let pending = Exchange {
        calls: Default::default(),
        gate: Some(gate.clone()),
        failure: false,
    };
    let cancellation = Cancellation::new();
    let wakes = Arc::new(WakeCounter::default());
    let mut future = pending.exchange(
        SecretString::new("STATE_SENTINEL".into()),
        cancellation.clone(),
    );
    assert!(poll(future.as_mut(), &wakes).is_pending());
    cancellation.cancel();
    assert!(matches!(
        poll(future.as_mut(), &wakes),
        std::task::Poll::Ready(Err(Failure::Cancelled))
    ));
    drop(future);
    assert_eq!(gate.1.load(Ordering::SeqCst), 1);
    assert_eq!(pending.calls.load(Ordering::SeqCst), 1);
}
