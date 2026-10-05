mod support;
use maestro_credentials::*;
use maestro_models::*;
use std::sync::Arc;
use support::*;

#[test]
fn adapters_share_current_format_reads_and_preservation() {
    let scratch = Scratch::new();
    let initial = r#"{"selected":{"type":"api_key","key":"initial","unknown":[1,true]},"alien":{"type":"unrecognized","payload":{"a":2}}}"#;
    for (storage, backing, writable) in adapters(&scratch.0, initial) {
        replace(backing.as_ref(), initial);
        let credentials = owner(storage.clone());
        assert_eq!(credentials.list(), vec!["alien", "selected"]);
        assert_secret(
            &block_on(credentials.resolve("selected".into(), Cancellation::new())).unwrap(),
            "initial",
            "stored",
        );
        let external = r#"{"selected":{"type":"api_key","key":"external","unknown":[1,true]},"alien":{"type":"unrecognized","payload":{"a":2}},"external":123}"#;
        replace(backing.as_ref(), external);
        assert_eq!(credentials.list(), vec!["alien", "selected"]);
        credentials.reload(&Cancellation::new()).unwrap();
        assert_eq!(credentials.list(), vec!["alien", "external", "selected"]);
        let result = credentials.set(
            "new",
            Credential::ApiKey {
                value: secret("new-value"),
            },
            &Cancellation::new(),
        );
        if writable {
            result.unwrap();
            let current: serde_json::Value =
                serde_json::from_str(&bytes(backing.as_ref())).unwrap();
            assert_eq!(current["selected"]["unknown"], serde_json::json!([1, true]));
            assert_eq!(current["alien"]["payload"], serde_json::json!({"a":2}));
            assert_eq!(current["external"], 123);
            credentials.remove("new", &Cancellation::new()).unwrap();
            assert!(!credentials.list().contains(&"new".into()));
        } else {
            assert_eq!(result, Err(CredentialError::ReadOnly));
            assert_eq!(
                credentials.remove("selected", &Cancellation::new()),
                Err(CredentialError::ReadOnly)
            );
            assert_eq!(bytes(backing.as_ref()), external);
        }
    }
}

#[test]
fn read_only_rejects_changes_without_mutation() {
    let scratch = Scratch::new();
    let initial = r#"{"selected":{"type":"api_key","key":"key"}}"#;
    for (storage, backing, writable) in adapters(&scratch.0, initial) {
        if writable {
            continue;
        }
        replace(backing.as_ref(), initial);
        let credentials = owner(storage.clone());
        assert_eq!(
            credentials.set(
                "new",
                Credential::ApiKey {
                    value: secret("new")
                },
                &Cancellation::new()
            ),
            Err(CredentialError::ReadOnly)
        );
        assert_eq!(
            credentials.remove("selected", &Cancellation::new()),
            Err(CredentialError::ReadOnly)
        );
        for value in [initial, "replacement"] {
            assert_eq!(
                storage.transact(&Cancellation::new(), &mut |_| Ok(Some(secret(value)))),
                Err(CredentialError::ReadOnly)
            );
        }
        assert_eq!(
            storage.transact(&Cancellation::new(), &mut |_| Err(
                CredentialError::Malformed
            )),
            Err(CredentialError::Malformed)
        );
        storage
            .transact(&Cancellation::new(), &mut |_| Ok(None))
            .unwrap();
        assert_eq!(bytes(backing.as_ref()), initial);
        assert_secret(
            &block_on(credentials.resolve("selected".into(), Cancellation::new())).unwrap(),
            "key",
            "stored",
        );
    }
}

#[test]
fn reload_failure_preserves_last_valid_snapshot() {
    let initial = r#"{"selected":{"type":"api_key","key":"valid"}}"#;
    let storage = CountingStorage::new(Arc::new(MemoryCredentialStorage::new(Some(secret(
        initial,
    )))));
    let credentials = owner(storage.clone());
    storage
        .fail
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        credentials.reload(&Cancellation::new()),
        Err(CredentialError::Storage)
    );
    assert_eq!(credentials.list(), vec!["selected"]);
    assert_eq!(
        request(credentials.clone(), "selected").0.failure,
        Some(Failure::AuthenticationFailed)
    );
    storage
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);
    credentials.reload(&Cancellation::new()).unwrap();
    assert_eq!(request(credentials, "selected").0.failure, None);
    let scratch = Scratch::new();
    let path = scratch.0.join("credentials.json");
    let file = Arc::new(FileCredentialStorage::new(path.clone()).unwrap());
    replace(file.as_ref(), initial);
    let credentials = owner(file.clone());
    for malformed in ["", "invalid-SENTINEL", "[]", "null"] {
        std::fs::write(&path, malformed).unwrap();
        assert_eq!(
            credentials.reload(&Cancellation::new()),
            Err(CredentialError::Malformed)
        );
        assert_eq!(credentials.list(), vec!["selected"]);
        assert_eq!(
            credentials.set(
                "new",
                Credential::ApiKey {
                    value: secret("new")
                },
                &Cancellation::new()
            ),
            Err(CredentialError::Malformed)
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), malformed);
        assert_eq!(
            Credentials::new(file.clone(), options()).err(),
            Some(CredentialError::Malformed)
        );
    }
    std::fs::write(&path, [0xff]).unwrap();
    assert_eq!(
        credentials.reload(&Cancellation::new()),
        Err(CredentialError::Malformed)
    );
    assert_eq!(credentials.list(), vec!["selected"]);
    assert_eq!(std::fs::read(&path).unwrap(), vec![0xff]);
    std::fs::write(&path, r#"{"recovered":{"type":"api_key","key":"key"}}"#).unwrap();
    credentials.reload(&Cancellation::new()).unwrap();
    assert_eq!(credentials.list(), vec!["recovered"]);
}

#[test]
fn same_caller_accepts_storage_adapter_swaps() {
    let scratch = Scratch::new();
    for (storage, _, _) in adapters(
        &scratch.0,
        r#"{"selected":{"type":"api_key","key":"same"}}"#,
    ) {
        let (result, adapter) = request(owner(storage), "selected");
        assert_eq!(result.failure, None);
        assert_secret(&adapter.calls()[0].options.auth, "same", "stored");
    }
    for storage in [
        memory() as Arc<dyn CredentialStorage>,
        Arc::new(FileCredentialStorage::new(scratch.0.join("shared.json")).unwrap())
            as Arc<dyn CredentialStorage>,
    ] {
        let first = owner(storage.clone());
        let second = owner(storage);
        first
            .set(
                "first",
                Credential::ApiKey {
                    value: secret("one"),
                },
                &Cancellation::new(),
            )
            .unwrap();
        second
            .set(
                "second",
                Credential::ApiKey {
                    value: secret("two"),
                },
                &Cancellation::new(),
            )
            .unwrap();
        first.reload(&Cancellation::new()).unwrap();
        assert_eq!(first.list(), vec!["first", "second"]);
        assert_eq!(second.list(), first.list());
    }
}

#[test]
fn cancelled_storage_waiter_cannot_release_writer() {
    struct Announce {
        inner: Arc<dyn CredentialStorage>,
        entered: std::sync::mpsc::Sender<()>,
    }
    impl CredentialStorage for Announce {
        fn transact(
            &self,
            cancellation: &Cancellation,
            edit: &mut CredentialTransaction<'_>,
        ) -> Result<(), CredentialError> {
            self.entered.send(()).unwrap();
            self.inner.transact(cancellation, edit)
        }
    }
    let backing = memory();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let announced: Arc<dyn CredentialStorage> = Arc::new(Announce {
        inner: backing.clone(),
        entered: entered_tx,
    });
    let credentials = owner(announced.clone());
    entered_rx.recv().unwrap();
    let (owned_tx, owned_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let owning_storage = backing.clone();
    let writer = std::thread::spawn(move || {
        owning_storage.transact(&Cancellation::new(), &mut |_| {
            owned_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(Some(secret(
                r#"{"owner":{"type":"api_key","key":"owned"}}"#,
            )))
        })
    });
    owned_rx.recv().unwrap();
    let signal = Cancellation::new();
    let request_signal = signal.clone();
    let fake = scripted(1);
    let adapter = fake.clone();
    let mut models = Models::new(Arc::new(|| 123));
    models.register(model("chosen"), fake).unwrap();
    let request = std::thread::spawn(move || {
        block_on(models.complete(
            model("chosen"),
            context(),
            StreamOptions {
                auth_resolver: Some(credentials),
                cancellation: request_signal,
                ..Default::default()
            },
        ))
    });
    entered_rx.recv().unwrap();
    signal.cancel();
    assert_eq!(request.join().unwrap().failure, Some(Failure::Cancelled));
    assert!(adapter.calls().is_empty());
    let (third_tx, third_rx) = std::sync::mpsc::channel();
    let third = std::thread::spawn(move || {
        announced.transact(&Cancellation::new(), &mut |_| {
            third_tx.send(()).unwrap();
            Ok(None)
        })
    });
    entered_rx.recv().unwrap();
    assert!(matches!(
        third_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    release_tx.send(()).unwrap();
    writer.join().unwrap().unwrap();
    third_rx.recv().unwrap();
    third.join().unwrap().unwrap();
    assert!(bytes(backing.as_ref()).contains("owner"));
}
