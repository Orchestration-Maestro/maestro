//! Authentication uses caller-supplied effects on the original retained catalog.
use crate::{
    local_resources::Pump,
    tests::{model_support, session},
};
use maestro_credentials::{ApiKeyCredential, AuthCredential};
use std::{cell::RefCell, rc::Rc};

/// Controlled effects whose caller-visible log excludes ambient environment and shells.
struct Operations(Rc<RefCell<Vec<String>>>);

impl maestro_credentials::ConfigValueOperations for Operations {
    fn environment(&self, name: &str) -> Option<String> {
        self.0.borrow_mut().push(name.into());
        Some(
            match name {
                "BRIDGE_KEY" => " caller key ",
                "replacement-source" => " replacement key ",
                _ => panic!("unexpected credential source"),
            }
            .into(),
        )
    }
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        panic!("this credential source requests no shell");
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
            let (session, _) = session::runner_session("/auth");
            let catalog = session.model_registry;
            catalog.auth_storage().set(
                "fixture",
                AuthCredential::ApiKey(ApiKeyCredential {
                    key: "BRIDGE_KEY".into(),
                }),
            );
            let calls = Rc::new(RefCell::new(Vec::new()));
            let (mut pump, admission) = Pump::new(Rc::new(Operations(calls.clone())));
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
                    calls.borrow().as_slice(),
                    ["BRIDGE_KEY"],
                    "credential effects must start at consumption"
                );
            }
            catalog.auth_storage().set(
                "fixture",
                AuthCredential::ApiKey(ApiKeyCredential {
                    key: "replacement-source".into(),
                }),
            );
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            assert_eq!(calls.borrow().as_slice(), ["BRIDGE_KEY"]);
            assert_eq!(std::sync::Arc::strong_count(&catalog), 1);
        });
}

/// Checks the supplied result without exposing credential material in failure output.
fn check_auth(resolved: maestro_catalog::ResolvedRequestAuth) {
    let maestro_catalog::ResolvedRequestAuth::Ok { api_key, headers } = resolved else {
        panic!("expected controlled credential success");
    };
    assert!(api_key.as_deref().eq(&Some(" caller key ")), "selected key");
    assert!(headers.is_none());
}
