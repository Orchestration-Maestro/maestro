//! Borrowed credential callbacks use the original native trait without extending its lifetime.
use crate::{
    guest_calls::{Call, Callbacks, credentials::Backend},
    lifecycle::replies::Replies,
    local_resources::Pump,
};
use maestro_extensions::AuthStorageBackend;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Waker},
};

#[test]
fn credential_backend_keeps_borrowed_updates_pending_on_original_pump() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let (mut pump, backend, mut incoming) = native_backend();
            let registry = pump.registry();
            assert_original_thread(&backend, &mut incoming);
            let (release, blocked) = tokio::sync::oneshot::channel();
            let started = Arc::new(AtomicBool::new(false));
            let seen = started.clone();
            let mut borrowed = String::new();
            let output = &mut borrowed;
            let mut work = backend.with_lock_async(Box::new(move |current| {
                Box::pin(async move {
                    seen.store(true, Ordering::SeqCst);
                    *output = current.unwrap();
                    blocked.await.unwrap();
                    Ok(Some(String::new()))
                })
            }));
            let mut context = Context::from_waker(Waker::noop());
            assert!(work.as_mut().poll(&mut context).is_pending());
            let Call::Credential(crate::guest_calls::credentials::CredentialCall::Backend(call)) =
                incoming.try_recv().unwrap()
            else {
                panic!("backend callback missing")
            };
            assert!(call.asynchronous);
            let mut update =
                std::pin::pin!(call.update.update_async(Some("actual current text".into())));
            assert!(update.as_mut().poll(&mut context).is_pending());
            assert!(pump.answer_next().await);
            assert!(work.as_mut().poll(&mut context).is_pending());
            assert!(started.load(Ordering::SeqCst));
            let alias = registry
                .credentials(
                    Arc::new(maestro_extensions::AuthStorage::in_memory(
                        maestro_extensions::AuthStorageData::default(),
                    )),
                    super::session::runner_session("/unrelated")
                        .0
                        .model_registry
                        .config_values()
                        .clone(),
                )
                .unwrap();
            drop(alias);
            assert!(pump.answer_next().await);
            assert!(work.as_mut().poll(&mut context).is_pending());
            release.send(()).unwrap();
            assert!(work.as_mut().poll(&mut context).is_pending());
            assert_eq!(update.await.unwrap(), Some(String::new()));
            call.reply.send(Ok(())).unwrap();
            work.await.unwrap();
            assert_eq!(borrowed, "actual current text");
        });
}

#[test]
fn provider_boundary_rejections_leave_other_resources_usable() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("not-created.json");
    let runtime = maestro_extensions::create_extension_runtime::<maestro_extensions::Local>();
    let (_owner, api) = super::api::api(runtime.clone());
    let values = super::session::runner_session("/boundary")
        .0
        .model_registry
        .config_values()
        .clone();
    runtime.set_catalog_construction(maestro_extensions::CatalogConstruction {
        auth_path: Arc::new(|| Err("default path must remain lazy".into())),
        models_path: Arc::new(|| Err("default path must remain lazy".into())),
        config_values: values.clone(),
    });
    let (first, _first_admission) = Pump::new(crate::tests::support::widgets());
    let (other, _other_admission) = Pump::new(crate::tests::support::widgets());
    let accepted = Arc::new(maestro_extensions::AuthStorage::in_memory(
        maestro_extensions::AuthStorageData::default(),
    ));
    accepted.set_runtime_api_key("valid", "kept");
    let endpoint = first
        .registry()
        .credentials(accepted.clone(), values)
        .unwrap();
    assert_eq!(
        other.registry().credential_owner(&endpoint).err().unwrap(),
        "extension credential resource belongs to another host"
    );
    assert!(
        first
            .registry()
            .credential_owner(&endpoint)
            .unwrap()
            .0
            .has_auth("valid")
            .unwrap()
    );
    rejected_catalog_keeps_paths_lazy(
        &api,
        &other.registry(),
        endpoint.clone(),
        "extension credential resource belongs to another host",
    );
    runtime.invalidate(Some("literal constructor rejection".into()));
    assert_eq!(
        api.create_auth_storage(path.to_str()).err(),
        Some("literal constructor rejection".into())
    );
    assert!(!path.exists());
    assert!(accepted.has_auth("valid").unwrap());
    rejected_json_keeps_credentials(&accepted, &first.registry());
    rejected_catalog_keeps_paths_lazy(
        &api,
        &other.registry(),
        endpoint.clone(),
        "literal constructor rejection",
    );
    released_credentials_reject(endpoint, first);
}

/// Malformed transport text is rejected before accepting any credential mutation.
fn rejected_json_keeps_credentials(
    owner: &maestro_extensions::AuthStorage,
    registry: &crate::local_resources::Registry,
) {
    let (reply, result) = std::sync::mpsc::channel();
    crate::credential_reads::Request::Set("valid".into(), "{".into(), reply)
        .answer(owner, registry);
    assert_eq!(
        result.recv().unwrap(),
        Err("EOF while parsing an object at line 1 column 1".into())
    );
    assert!(owner.get_all().is_empty());
    assert!(owner.has_auth("valid").unwrap());
}

/// Provenance and stale facade rejection precede file selection and creation.
fn rejected_catalog_keeps_paths_lazy(
    api: &maestro_extensions::ExtensionAPI<maestro_extensions::Local>,
    registry: &crate::local_resources::Registry,
    endpoint: crate::credential_reads::Endpoint,
    expected: &str,
) {
    let (reply, result) = std::sync::mpsc::channel();
    crate::construction::Request::Catalog(endpoint, crate::construction::Source::File(None), reply)
        .answer(api, registry);
    assert_eq!(result.recv().unwrap().err().as_deref(), Some(expected));
}

/// A retired native capability rejects subsequent work without invalidating a sibling.
fn released_credentials_reject(endpoint: crate::credential_reads::Endpoint, mut pump: Pump) {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let registry = pump.registry();
            let survivor = registry
                .credentials(
                    Arc::new(maestro_extensions::AuthStorage::in_memory(
                        maestro_extensions::AuthStorageData::default(),
                    )),
                    registry.credential_owner(&endpoint).unwrap().1,
                )
                .unwrap();
            let route = endpoint
                .0
                .route::<()>(|id, _reply| crate::local_resources::Message::Release(id));
            assert!(pump.answer_next().await);
            drop(route);
            assert_eq!(
                registry.credential_owner(&endpoint).err().as_deref(),
                Some("extension credential resource ended")
            );
            let valid = survivor.clone();
            let worker = std::thread::spawn(move || {
                use crate::read_imports::ReadEndpoint;
                assert_eq!(
                    endpoint.read(crate::credential_reads::Request::List),
                    Err("extension credential resource ended".into())
                );
                valid.read(crate::credential_reads::Request::List)
            });
            assert!(pump.answer_next().await);
            assert!(pump.answer_next().await);
            assert_eq!(worker.join().unwrap(), Ok(Vec::<String>::new()));
            assert!(
                registry
                    .credential_owner(&survivor)
                    .unwrap()
                    .0
                    .list()
                    .is_empty()
            );
        });
}

/// Distinguishable configured-command effects supplied by the selected credential resource.
struct ConstructorEffects(&'static str);
impl maestro_extensions::ConfigValueOperations for ConstructorEffects {
    fn execute(&self, command: &str) -> Option<Vec<u8>> {
        assert_eq!(command, "constructor-effect");
        Some(self.0.as_bytes().to_vec())
    }
}
#[test]
fn catalog_constructors_keep_selected_credential_command_effects() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runtime =
                maestro_extensions::create_extension_runtime::<maestro_extensions::Local>();
            let (_owner, api) = super::api::api(runtime.clone());
            runtime.set_catalog_construction(maestro_extensions::CatalogConstruction {
                auth_path: Arc::new(|| Err("unexpected auth default".into())),
                models_path: Arc::new(|| Err("unexpected model default".into())),
                config_values: Arc::new(ConstructorEffects("facade effects")),
            });
            for source in [
                crate::construction::Source::File(Some(String::new())),
                crate::construction::Source::Memory,
            ] {
                let (pump, admission) = Pump::new(crate::tests::support::widgets());
                let registry = pump.registry();
                let owner = Arc::new(maestro_extensions::AuthStorage::in_memory(
                    maestro_extensions::AuthStorageData::default(),
                ));
                owner.set(
                    "provider",
                    maestro_extensions::AuthCredential::ApiKey(
                        maestro_credentials::ApiKeyCredential {
                            env: None,
                            key: "!constructor-effect".into(),
                        },
                    ),
                );
                let auth = registry
                    .credentials(owner, Arc::new(ConstructorEffects("credential effects")))
                    .unwrap();
                let (reply, result) = std::sync::mpsc::channel();
                crate::construction::Request::Catalog(auth, source, reply).answer(&api, &registry);
                let catalog = result.recv().unwrap().unwrap();
                let worker = std::thread::spawn(move || {
                    tokio::runtime::Builder::new_current_thread()
                        .build()
                        .unwrap()
                        .block_on(catalog.provider_key("provider".into()))
                });
                drop(admission);
                pump.serve().await;
                assert_eq!(
                    worker.join().unwrap().unwrap().as_deref(),
                    Some("credential effects")
                );
            }
        });
}

/// Creates a retained backend on its actual native request pump.
fn native_backend() -> (
    Pump,
    Arc<Backend>,
    tokio::sync::mpsc::UnboundedReceiver<Call>,
) {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let (_owner, api) = super::api::api(maestro_extensions::create_extension_runtime::<
        maestro_extensions::Local,
    >());
    let endpoint = registry.api(api).unwrap();
    let (calls, incoming) = tokio::sync::mpsc::unbounded_channel();
    let mut callbacks = Callbacks::new(calls.downgrade(), Replies::default());
    let id = callbacks.allocate().unwrap().id;
    let handler = callbacks.retain(id, admission).unwrap();
    let backend = Arc::new(Backend {
        handler,
        owner: endpoint.0,
    });
    (pump, backend, incoming)
}
/// Off-thread native calls fail before invoking the borrowed update or entering the guest.
fn assert_original_thread(
    backend: &Arc<Backend>,
    incoming: &mut tokio::sync::mpsc::UnboundedReceiver<Call>,
) {
    let off_thread = backend.clone();
    let failure = std::thread::spawn(move || {
        off_thread.with_lock(&mut |_| panic!("off-thread update invoked"))
    })
    .join()
    .unwrap()
    .unwrap_err();
    assert_eq!(
        failure.to_string(),
        "extension credential backend requires its original pump thread"
    );
    assert!(incoming.try_recv().is_err());
}
