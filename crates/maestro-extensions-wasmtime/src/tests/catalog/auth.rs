//! Authentication uses caller-supplied effects on the original retained catalog.
use crate::{
    local_resources::Pump,
    tests::{model_support, session},
};
use maestro_credentials::{ApiKeyCredential, AuthCredential};
use std::sync::{Arc, Mutex};

/// Controlled effects whose caller-visible log excludes ambient environment and shells.
struct Operations(Arc<Mutex<Vec<String>>>);

impl maestro_credentials::ConfigValueOperations for Operations {
    fn execute(&self, name: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().push(name.into());
        Some(
            match name {
                "bridge-key" => " caller key ",
                "replacement-source" => " replacement key ",
                _ => panic!("unexpected credential source"),
            }
            .as_bytes()
            .to_vec(),
        )
    }
}

#[test]
fn catalog_auth_uses_explicit_local_credential_effects() {
    run_auth(false);
}

#[test]
fn credential_effects_start_during_consumption() {
    run_auth(true);
}

/// Checks both the call-time origin and its separately observable credential result.
fn run_auth(assert_started: bool) {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let (session, _) = session::runner_session_with_config(
                "/auth",
                maestro_models::SharedHandle::new(Operations(calls.clone())),
            );
            let catalog = session.model_registry;
            catalog.auth_storage().set(
                "fixture",
                AuthCredential::ApiKey(ApiKeyCredential {
                    env: None,
                    key: "!bridge-key".into(),
                }),
            );
            let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump.registry().catalog(catalog.clone()).unwrap();
            let model = model_support::model("auth-owned").read().unwrap().clone();
            let worker = std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let resolved = endpoint.auth(model).await.unwrap();
                        check_auth(resolved);
                    });
            });
            assert!(pump.answer_next().await);
            if assert_started {
                assert_eq!(
                    calls.lock().unwrap().as_slice(),
                    ["bridge-key"],
                    "credential effects must start at consumption"
                );
            }
            catalog.auth_storage().set(
                "fixture",
                AuthCredential::ApiKey(ApiKeyCredential {
                    env: None,
                    key: "!replacement-source".into(),
                }),
            );
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            assert_eq!(calls.lock().unwrap().as_slice(), ["bridge-key"]);
            assert_eq!(std::sync::Arc::strong_count(&catalog), 1);
        });
}

/// Checks the supplied result without exposing credential material in failure output.
fn check_auth(resolved: maestro_catalog::ResolvedRequestAuth) {
    let maestro_catalog::ResolvedRequestAuth::Ok { api_key, headers } = resolved else {
        panic!("expected controlled credential success");
    };
    assert!(api_key.as_deref().eq(&Some("caller key")), "selected key");
    assert!(headers.is_none());
}
