//! Reusable assertions for adapter test suites; violations panic.

use crate::{Record, SessionHeader, Storage, StorageError};

/// Checks explicit creation, ordered records, lookup and independent close/reopen.
pub fn create_append_read_open(storage: &dyn Storage) {
    let header = SessionHeader {
        session_id: "session".into(),
        data: vec![1, 2],
    };
    let handle = storage.create(header.clone()).unwrap();
    let empty = handle.read().unwrap();
    assert_eq!(empty.metadata.header, header);
    assert!(empty.records.is_empty());
    assert_eq!(empty.selected_position, None);
    let records = vec![
        Record {
            id: "a".into(),
            data: vec![3],
        },
        Record {
            id: "b".into(),
            data: vec![4],
        },
    ];
    handle.append(records.clone(), Some("b".into())).unwrap();
    let snapshot = handle.read().unwrap();
    assert_eq!(snapshot.metadata.header, header);
    assert_eq!(snapshot.records, records);
    assert_eq!(snapshot.selected_position.as_deref(), Some("b"));
    assert_eq!(handle.get("a").unwrap(), Some(records[0].clone()));
    assert_eq!(handle.get("missing").unwrap(), None);
    assert_eq!(storage.list().unwrap(), vec![snapshot.metadata.clone()]);
    let independent = storage.open("session").unwrap();
    handle.close().unwrap();
    assert_eq!(handle.read(), Err(StorageError::Closed));
    assert_eq!(handle.get("a"), Err(StorageError::Closed));
    assert_eq!(handle.append(vec![], None), Err(StorageError::Closed));
    assert_eq!(handle.select(None), Err(StorageError::Closed));
    handle.close().unwrap();
    assert_eq!(independent.read().unwrap(), snapshot);
    independent.close().unwrap();
    assert_eq!(storage.list().unwrap(), vec![snapshot.metadata.clone()]);
    assert_eq!(storage.open("session").unwrap().read().unwrap(), snapshot);
}

fn header(id: &str) -> SessionHeader {
    SessionHeader {
        session_id: id.into(),
        data: vec![0, 255],
    }
}
fn record(id: &str) -> Record {
    Record {
        id: id.into(),
        data: id.as_bytes().to_vec(),
    }
}

/// Checks selected positions and complete preservation after definite rejection.
pub fn selected_position_and_rejection(storage: &dyn Storage) {
    let handle = storage.create(header("s")).unwrap();
    handle
        .append(vec![record("a"), record("b")], Some("b".into()))
        .unwrap();
    handle.select(Some("a".into())).unwrap();
    assert_eq!(
        handle.read().unwrap().selected_position.as_deref(),
        Some("a")
    );
    handle.select(None).unwrap();
    assert_eq!(handle.read().unwrap().selected_position, None);
    handle.append(vec![], Some("b".into())).unwrap();
    let before = handle.read().unwrap();
    let other = storage.create(header("other")).unwrap();
    other
        .append(vec![record("foreign")], Some("foreign".into()))
        .unwrap();
    for (records, position) in [
        (vec![record("c"), record("c")], Some("c".into())),
        (vec![record("c"), record("a")], Some("c".into())),
        (vec![record("c")], Some("missing".into())),
        (vec![record("c")], Some("foreign".into())),
    ] {
        assert!(matches!(
            handle.append(records, position),
            Err(StorageError::Rejected { .. })
        ));
        assert_eq!(handle.read().unwrap(), before);
    }
    assert!(matches!(
        handle.select(Some("foreign".into())),
        Err(StorageError::Rejected { .. })
    ));
    assert_eq!(handle.read().unwrap(), before);
}

/// Checks identity-local records, duplicate creation and unknown opens.
pub fn identity_isolation(storage: &dyn Storage) {
    let a = storage.create(header("a")).unwrap();
    let b = storage.create(header("b")).unwrap();
    let left = Record {
        id: "same".into(),
        data: vec![1],
    };
    let right = Record {
        id: "same".into(),
        data: vec![2],
    };
    a.append(vec![left.clone()], Some("same".into())).unwrap();
    b.append(vec![right.clone()], None).unwrap();
    assert_eq!(a.get("same").unwrap(), Some(left));
    assert_eq!(b.get("same").unwrap(), Some(right));
    let before = b.read().unwrap();
    a.select(None).unwrap();
    a.append(vec![record("only-a")], Some("only-a".into()))
        .unwrap();
    assert_eq!(b.get("only-a").unwrap(), None);
    assert!(matches!(
        b.select(Some("only-a".into())),
        Err(StorageError::Rejected { .. })
    ));
    assert!(matches!(
        storage.create(header("b")),
        Err(StorageError::Rejected { .. })
    ));
    assert!(matches!(
        storage.open("missing"),
        Err(StorageError::Rejected { .. })
    ));
    a.close().unwrap();
    assert_eq!(b.read().unwrap(), before);
    assert_eq!(storage.open("b").unwrap().read().unwrap(), before);
    assert_eq!(storage.list().unwrap().len(), 2);
}

/// Checks nested ownership of snapshots, lookup results and metadata listings.
pub fn detached_snapshots(storage: &dyn Storage) {
    let handle = storage.create(header("s")).unwrap();
    handle.append(vec![record("a")], Some("a".into())).unwrap();
    let before = handle.read().unwrap();
    let mut changed = before.clone();
    changed.metadata.header.data.clear();
    changed.metadata.header.session_id.clear();
    changed.metadata.persistent_locator = Some("changed".into());
    changed.metadata.resumable = !changed.metadata.resumable;
    changed.records[0].data.clear();
    changed.records[0].id.clear();
    changed.selected_position = None;
    let mut listed = storage.list().unwrap();
    listed[0].header.data.clear();
    listed[0].persistent_locator = Some("changed".into());
    let mut found = handle.get("a").unwrap().unwrap();
    found.data.clear();
    assert_eq!(handle.read().unwrap(), before);
    assert_eq!(storage.list().unwrap(), vec![before.metadata.clone()]);
    handle.append(vec![record("b")], None).unwrap();
    handle.select(Some("b".into())).unwrap();
    assert_eq!(before.records, vec![record("a")]);
    assert_eq!(before.selected_position.as_deref(), Some("a"));
}

/// Checks byte-exact retention without decoding unfamiliar encodings.
pub fn opaque_payloads(storage: &dyn Storage) {
    let header = SessionHeader {
        session_id: String::new(),
        data: vec![255, 0, 128],
    };
    let records = vec![
        Record {
            id: String::new(),
            data: vec![],
        },
        Record {
            id: "alien".into(),
            data: vec![0, 255, 128, 123],
        },
    ];
    let handle = storage.create(header.clone()).unwrap();
    handle.append(records.clone(), Some(String::new())).unwrap();
    let snapshot = handle.read().unwrap();
    assert_eq!(snapshot.metadata.header, header);
    assert_eq!(snapshot.records, records);
    assert_eq!(handle.get("").unwrap(), Some(records[0].clone()));
}

/// Checks concurrent batches and position changes against valid serial outcomes.
pub fn serial_mutations(storage: &dyn Storage) {
    let handle = storage.create(header("s")).unwrap();
    handle
        .append(vec![record("seed")], Some("seed".into()))
        .unwrap();
    let a = storage.open("s").unwrap();
    let b = storage.open("s").unwrap();
    let selector = storage.open("s").unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    std::thread::scope(|scope| {
        for (writer, ids) in [(a, ["a1", "a2"]), (b, ["b1", "b2"])] {
            let barrier = barrier.clone();
            scope.spawn(move || {
                barrier.wait();
                writer
                    .append(ids.map(record).to_vec(), Some(ids[1].into()))
                    .unwrap();
            });
        }
        let start = barrier.clone();
        scope.spawn(move || {
            start.wait();
            selector.select(Some("seed".into())).unwrap();
        });
        barrier.wait();
    });
    let snapshot = handle.read().unwrap();
    let ids: Vec<_> = snapshot
        .records
        .iter()
        .map(|record| record.id.as_str())
        .collect();
    assert!(ids == ["seed", "a1", "a2", "b1", "b2"] || ids == ["seed", "b1", "b2", "a1", "a2"]);
    let last = ids.last().copied().unwrap();
    assert!(
        snapshot.selected_position.as_deref() == Some(last)
            || snapshot.selected_position.as_deref() == Some("seed")
    );
}

/// Deterministic scheduling witnesses for real adapter paths in test suites.
pub trait Controls {
    /// Pauses an admitted append after its first staged record, before publication,
    /// without holding the snapshot lock.
    fn pause_next_append(&self, session_id: &str) -> Pause;
    /// Observes admission closing immediately before admitted-write draining.
    fn observe_close(&self, session_id: &str) -> std::sync::mpsc::Receiver<()>;
}

/// Owned endpoints for a paused real append.
pub struct Pause {
    /// Receives the admission/preparation witness.
    pub reached: std::sync::mpsc::Receiver<()>,
    /// Sending releases the admitted write.
    pub release: std::sync::mpsc::Sender<()>,
}

/// Checks that readers never observe a staged prefix or mismatched position.
pub fn whole_batch_visibility(storage: &dyn Storage, controls: &dyn Controls) {
    let handle = storage.create(header("s")).unwrap();
    handle
        .append(vec![record("old")], Some("old".into()))
        .unwrap();
    let reader = storage.open("s").unwrap();
    let before = reader.read().unwrap();
    let pause = controls.pause_next_append("s");
    std::thread::scope(|scope| {
        let write = scope.spawn(|| handle.append(vec![record("a"), record("b")], Some("b".into())));
        pause.reached.recv().unwrap();
        assert_eq!(reader.read().unwrap(), before);
        assert_eq!(reader.get("a").unwrap(), None);
        pause.release.send(()).unwrap();
        write.join().unwrap().unwrap();
    });
    let after = reader.read().unwrap();
    assert_eq!(after.records, vec![record("old"), record("a"), record("b")]);
    assert_eq!(after.selected_position.as_deref(), Some("b"));
}

/// Checks close's real admission/drain path and independent handle usability.
pub fn close_settles_admitted_writes(storage: &dyn Storage, controls: &dyn Controls) {
    let handle = storage.create(header("s")).unwrap();
    let reader = storage.open("s").unwrap();
    let pause = controls.pause_next_append("s");
    let closing = controls.observe_close("s");
    let (done_tx, done) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let writer = handle.clone();
        let write =
            scope.spawn(move || writer.append(vec![record("a"), record("b")], Some("b".into())));
        pause.reached.recv().unwrap();
        let closer = handle.clone();
        let close = scope.spawn(move || {
            let result = closer.close();
            done_tx.send(()).unwrap();
            result
        });
        closing.recv().unwrap();
        assert_eq!(done.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty));
        assert_eq!(handle.read(), Err(StorageError::Closed));
        assert_eq!(handle.get("a"), Err(StorageError::Closed));
        assert_eq!(
            handle.append(vec![record("late")], None),
            Err(StorageError::Closed)
        );
        assert_eq!(handle.select(None), Err(StorageError::Closed));
        assert!(reader.read().unwrap().records.is_empty());
        pause.release.send(()).unwrap();
        write.join().unwrap().unwrap();
        close.join().unwrap().unwrap();
        done.recv().unwrap();
    });
    handle.close().unwrap();
    assert_eq!(
        reader.read().unwrap().records,
        vec![record("a"), record("b")]
    );
    reader.append(vec![record("c")], Some("c".into())).unwrap();
    assert_eq!(
        storage
            .open("s")
            .unwrap()
            .read()
            .unwrap()
            .selected_position
            .as_deref(),
        Some("c")
    );
}

/// Checks uncertain outcomes disable mutations through the handle and its clones.
/// This makes no rejection, rollback or durability assertion.
pub fn uncertain_outcome_blocks_mutations(storage: &dyn Storage, arm_uncertain: &dyn Fn(&str)) {
    let handle = storage.create(header("s")).unwrap();
    handle
        .append(vec![record("old")], Some("old".into()))
        .unwrap();
    arm_uncertain("s");
    assert!(matches!(
        handle.append(vec![record("a"), record("b")], Some("b".into())),
        Err(StorageError::Uncertain { .. })
    ));
    let clone = handle.clone();
    for caller in [&handle, &clone] {
        assert!(caller.append(vec![record("late")], None).is_err());
        assert!(caller.select(None).is_err());
    }
    handle.close().unwrap();
    clone.close().unwrap();
}
