use maestro_storage::{MemoryStorage, conformance};

#[test]
fn create_append_read_open() {
    conformance::create_append_read_open(&MemoryStorage::new());
    conformance::create_append_read_open(&support::ControlledStorage::new());
}

#[test]
fn selected_position_and_rejection() {
    conformance::selected_position_and_rejection(&MemoryStorage::new());
    conformance::selected_position_and_rejection(&support::ControlledStorage::new());
}

#[test]
fn identity_isolation() {
    conformance::identity_isolation(&MemoryStorage::new());
    conformance::identity_isolation(&support::ControlledStorage::new());
}

#[test]
fn detached_snapshots() {
    conformance::detached_snapshots(&MemoryStorage::new());
    conformance::detached_snapshots(&support::ControlledStorage::new());
}

#[test]
fn opaque_payloads() {
    conformance::opaque_payloads(&MemoryStorage::new());
    conformance::opaque_payloads(&support::ControlledStorage::new());
}

#[test]
fn serial_mutations() {
    conformance::serial_mutations(&MemoryStorage::new());
    conformance::serial_mutations(&support::ControlledStorage::new());
}

#[test]
fn memory_is_explicitly_ephemeral() {
    use maestro_storage::{SessionHeader, Storage, StorageError};
    let store = MemoryStorage::new();
    let handle = store
        .create(SessionHeader {
            session_id: "s".into(),
            data: vec![],
        })
        .unwrap();
    let metadata = handle.read().unwrap().metadata;
    assert_eq!(metadata.persistent_locator, None);
    assert!(!metadata.resumable);
    handle.close().unwrap();
    assert_eq!(store.open("s").unwrap().read().unwrap().metadata, metadata);
    assert!(matches!(
        MemoryStorage::new().open("s"),
        Err(StorageError::Rejected { .. })
    ));
}

#[test]
fn memory_works_without_controlled_adapter() {
    conformance::create_append_read_open(&MemoryStorage::new());
}

mod support;
#[test]
fn adapter_substitution_preserves_callers() {
    fn caller(storage: &dyn maestro_storage::Storage) {
        conformance::create_append_read_open(storage);
    }
    caller(&MemoryStorage::new());
    caller(&support::ControlledStorage::new());
}

#[test]
fn whole_batch_visibility() {
    let store = support::ControlledStorage::new();
    conformance::whole_batch_visibility(&store, &store);
}

#[test]
fn close_settles_admitted_writes() {
    let store = support::ControlledStorage::new();
    conformance::close_settles_admitted_writes(&store, &store);
}

#[test]
fn uncertain_outcome_blocks_mutations() {
    for after_publication in [false, true] {
        let store = support::ControlledStorage::new();
        conformance::uncertain_outcome_blocks_mutations(&store, &|id| {
            store.arm_uncertain(id, after_publication)
        });
        use maestro_storage::{
            Record, SessionHeader, Storage, StorageError, conformance::Controls,
        };
        let store = support::ControlledStorage::new();
        let handle = store
            .create(SessionHeader {
                session_id: "pending".into(),
                data: vec![],
            })
            .unwrap();
        let pause = store.pause_next_append("pending");
        store.arm_uncertain("pending", after_publication);
        std::thread::scope(|scope| {
            let first = handle.clone();
            let fault = scope.spawn(move || {
                first.append(
                    vec![Record {
                        id: "a".into(),
                        data: vec![1],
                    }],
                    Some("a".into()),
                )
            });
            pause.reached.recv().unwrap();
            let observed = store.observe_next_mutation("pending");
            let next = handle.clone();
            let pending_append = scope.spawn(move || {
                next.append(
                    vec![Record {
                        id: "late".into(),
                        data: vec![],
                    }],
                    None,
                )
            });
            observed.recv().unwrap();
            let observed = store.observe_next_mutation("pending");
            let next = handle.clone();
            let pending_select = scope.spawn(move || next.select(None));
            observed.recv().unwrap();
            pause.release.send(()).unwrap();
            for result in [
                fault.join().unwrap(),
                pending_append.join().unwrap(),
                pending_select.join().unwrap(),
            ] {
                assert!(matches!(result, Err(StorageError::Uncertain { .. })));
            }
        });
        let snapshot = store.open("pending").unwrap().read().unwrap();
        if after_publication {
            assert_eq!(
                snapshot.records,
                vec![Record {
                    id: "a".into(),
                    data: vec![1]
                }]
            );
            assert_eq!(snapshot.selected_position.as_deref(), Some("a"));
        } else {
            assert!(snapshot.records.is_empty());
            assert_eq!(snapshot.selected_position, None);
        }
        handle.close().unwrap();
    }
}
