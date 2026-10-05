use super::*;
use crate::{ManifestSettings, Settings, SettingsTarget};
use serde_json::json;
use std::sync::{atomic::Ordering, mpsc};
use std::time::Duration;

struct Scratch {
    root: std::path::PathBuf,
}
impl Scratch {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        loop {
            let root = std::env::temp_dir().join(format!(
                "maestro-settings-native-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create scratch: {e}"),
            }
        }
    }
    fn locations(&self) -> SettingsLocations {
        SettingsLocations::new(
            self.root.join("cwd"),
            None,
            self.root.join("user"),
            self.root.join("home"),
        )
        .unwrap()
    }
    fn adapter(&self) -> FileSettingsStorage {
        FileSettingsStorage::new(self.locations(), Arc::new(AtomicBool::new(false)))
    }
    fn file(&self, scope: SettingsScope) -> std::path::PathBuf {
        self.locations()
            .configuration_directory(scope)
            .join("settings.json")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let result = std::fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}
fn settings(storage: FileSettingsStorage) -> Settings {
    Settings::new(
        Map::new(),
        ManifestSettings {
            values: Map::new(),
            locks: vec![],
        },
        Box::new(storage),
    )
    .unwrap()
}
fn set(s: &mut Settings, scope: SettingsScope) -> Result<crate::SettingsSnapshot, SettingsError> {
    s.set(SettingsTarget::Stored(scope), &["b".into()], json!(2))
}

#[test]
fn native_transactions_serialize_the_entire_update() {
    for scope in [SettingsScope::User, SettingsScope::Project] {
        for existing in [false, true] {
            let scratch = Scratch::new();
            if existing {
                scratch
                    .adapter()
                    .transact(scope, &mut |_| {
                        Ok(Some(json!({"initial":0}).as_object().unwrap().clone()))
                    })
                    .unwrap();
            }
            let (entered_tx, entered_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let (committed_tx, committed_rx) = mpsc::channel();
            let mut a = scratch.adapter();
            let a_thread = std::thread::spawn(move || {
                a.transact(scope, &mut |current| {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    let mut next = current.clone();
                    next.insert("a".into(), json!(1));
                    Ok(Some(next))
                })
                .unwrap();
                committed_tx.send(()).unwrap();
            });
            entered_rx.recv().unwrap();
            let mut b = scratch.adapter();
            let mut waits = 0;
            b.controls.wait = Some(Box::new(move |duration| {
                assert_eq!(duration, Duration::from_millis(20));
                assert_eq!(waits, 0);
                waits += 1;
                release_tx.send(()).unwrap();
                committed_rx.recv().unwrap();
            }));
            let mut caller = settings(b);
            let accepted = set(&mut caller, scope).unwrap();
            assert_eq!(accepted.values["a"], json!(1));
            assert_eq!(accepted.values["b"], json!(2));
            a_thread.join().unwrap();
            assert_eq!(settings(scratch.adapter()).resolve(), accepted);
            assert!(
                scratch
                    .file(scope)
                    .with_file_name("settings.json.lock")
                    .exists()
            );
        }
    }
}
#[test]
fn acquisition_uses_ten_attempts_and_twenty_millisecond_gaps() {
    for (busy, terminal, expected_calls, expected_waits, succeeds) in [
        (0, None, 1, 0, true),
        (9, None, 10, 9, true),
        (10, None, 10, 9, false),
        (0, Some(std::io::ErrorKind::PermissionDenied), 1, 0, false),
        (2, Some(std::io::ErrorKind::Interrupted), 3, 2, false),
    ] {
        let scratch = Scratch::new();
        let calls = Arc::new(std::sync::Mutex::new(0));
        let waits = Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut storage = scratch.adapter();
        let recorded_calls = calls.clone();
        storage.controls.acquire = Some(Box::new(move |file| {
            let mut calls = recorded_calls.lock().unwrap();
            *calls += 1;
            if *calls <= busy {
                Err(std::fs::TryLockError::WouldBlock)
            } else if let Some(kind) = terminal {
                Err(std::fs::TryLockError::Error(std::io::Error::from(kind)))
            } else {
                file.try_lock()
            }
        }));
        let recorded_waits = waits.clone();
        storage.controls.wait = Some(Box::new(move |duration| {
            recorded_waits.lock().unwrap().push(duration)
        }));
        let mut caller = settings(storage);
        let before = caller.resolve();
        let result = set(&mut caller, SettingsScope::User);
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(*calls.lock().unwrap(), expected_calls);
        assert_eq!(
            *waits.lock().unwrap(),
            vec![Duration::from_millis(20); expected_waits]
        );
        if !succeeds {
            let kind = terminal
                .map(SettingsFileError::Io)
                .unwrap_or(SettingsFileError::Contended);
            assert!(
                matches!(result,Err(SettingsError::File { kind: actual, .. }) if actual == kind)
            );
            assert_eq!(caller.resolve(), before);
            assert!(!scratch.file(SettingsScope::User).exists());
        }
    }
}
#[test]
fn cancelled_waiter_never_releases_an_admitted_writer() {
    let scratch = Scratch::new();
    let scope = SettingsScope::User;
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut a = scratch.adapter();
    let a_cancelled = a.cancelled.clone();
    let late_signal = a_cancelled.clone();
    let writer = std::thread::spawn(move || {
        a.transact(scope, &mut |_| {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            assert!(a_cancelled.load(Ordering::SeqCst));
            Ok(Some(json!({"a":1}).as_object().unwrap().clone()))
        })
        .unwrap();
    });
    entered_rx.recv().unwrap();
    let mut b = scratch.adapter();
    let cancelled = b.cancelled.clone();
    b.controls.wait = Some(Box::new(move |_| cancelled.store(true, Ordering::SeqCst)));
    let mut caller = settings(b);
    let before = caller.resolve();
    assert!(matches!(
        set(&mut caller, scope),
        Err(SettingsError::File {
            kind: SettingsFileError::Cancelled,
            ..
        })
    ));
    assert_eq!(caller.resolve(), before);
    let mut c = scratch.adapter();
    c.controls.wait = Some(Box::new(|_| {}));
    let mut called = false;
    assert!(matches!(
        c.transact(scope, &mut |_| {
            called = true;
            Ok(None)
        }),
        Err(SettingsError::File {
            kind: SettingsFileError::Contended,
            ..
        })
    ));
    assert!(!called);
    late_signal.store(true, Ordering::SeqCst);
    release_tx.send(()).unwrap();
    writer.join().unwrap();
    assert_eq!(settings(scratch.adapter()).resolve().values["a"], json!(1));
    let mut storage = scratch.adapter();
    let during_read = storage.cancelled.clone();
    storage.controls.failure = Some(Box::new(move |point| {
        if point == FailurePoint::Read {
            during_read.store(true, Ordering::SeqCst);
        }
        Ok(())
    }));
    let mut called = false;
    assert!(matches!(
        storage.transact(scope, &mut |_| {
            called = true;
            Ok(None)
        }),
        Err(SettingsError::File {
            kind: SettingsFileError::Cancelled,
            ..
        })
    ));
    assert!(!called);
    for after_acquisition in [false, true] {
        let mut storage = scratch.adapter();
        let signal = storage.cancelled.clone();
        if after_acquisition {
            storage.controls.acquire = Some(Box::new(move |file| {
                file.try_lock()?;
                signal.store(true, Ordering::SeqCst);
                Ok(())
            }));
        } else {
            signal.store(true, Ordering::SeqCst);
        }
        let mut called = false;
        assert!(matches!(
            storage.transact(scope, &mut |_| {
                called = true;
                Ok(None)
            }),
            Err(SettingsError::File {
                kind: SettingsFileError::Cancelled,
                ..
            })
        ));
        assert!(!called);
        let mut next = scratch.adapter();
        next.transact(scope, &mut |_| Ok(None)).unwrap();
        assert!(storage.cancelled.load(Ordering::SeqCst));
    }
}
#[test]
fn write_failures_preserve_snapshot_without_claiming_rollback() {
    for stage in [
        FailurePoint::Read,
        FailurePoint::BeforeWrite,
        FailurePoint::AfterTruncate,
    ] {
        let scratch = Scratch::new();
        let scope = SettingsScope::User;
        scratch
            .adapter()
            .transact(scope, &mut |_| {
                Ok(Some(json!({"keep":1}).as_object().unwrap().clone()))
            })
            .unwrap();
        let original = std::fs::read(scratch.file(scope)).unwrap();
        let mut storage = scratch.adapter();
        let fail = Arc::new(AtomicBool::new(false));
        let enabled = fail.clone();
        storage.controls.failure = Some(Box::new(move |point| {
            if enabled.load(Ordering::SeqCst) && point == stage {
                Err(std::io::Error::from(std::io::ErrorKind::Other))
            } else {
                Ok(())
            }
        }));
        let mut caller = settings(storage);
        let before = caller.resolve();
        fail.store(true, Ordering::SeqCst);
        assert!(matches!(
            set(&mut caller, scope),
            Err(SettingsError::File {
                kind: SettingsFileError::Io(std::io::ErrorKind::Other),
                ..
            })
        ));
        assert_eq!(caller.resolve(), before);
        let after = std::fs::read(scratch.file(scope)).unwrap();
        if stage == FailurePoint::AfterTruncate {
            assert!(after.is_empty());
        } else {
            assert_eq!(after, original);
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(scratch.file(scope).with_file_name("settings.json.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        drop(lock);
        std::fs::write(scratch.file(scope), &original).unwrap();
        fail.store(false, Ordering::SeqCst);
        let accepted = set(&mut caller, scope).unwrap();
        assert_eq!(settings(scratch.adapter()).resolve(), accepted);
        let settled = std::fs::read(scratch.file(scope)).unwrap();
        let mut storage = scratch.adapter();
        storage.transact(scope, &mut |_| Ok(None)).unwrap();
        assert!(
            storage
                .transact(scope, &mut |_| Err(SettingsError::Storage { scope }))
                .is_err()
        );
        assert_eq!(std::fs::read(scratch.file(scope)).unwrap(), settled);
    }
}
