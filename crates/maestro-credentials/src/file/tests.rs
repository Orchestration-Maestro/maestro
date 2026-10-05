use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "maestro-file-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("scratch creation failed: {error}"),
            }
        }
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[derive(Clone, Copy)]
enum Outcome {
    Busy,
    Success,
    Failed,
}
struct Controlled {
    outcomes: Mutex<VecDeque<Outcome>>,
    attempts: AtomicUsize,
    waits: Mutex<Vec<Duration>>,
    cancel_wait: bool,
}
impl Acquisition for Controlled {
    fn try_lock(&self, file: &File) -> Result<(), TryLockError> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        match self.outcomes.lock().unwrap().pop_front().unwrap() {
            Outcome::Busy => Err(TryLockError::WouldBlock),
            Outcome::Failed => Err(TryLockError::Error(std::io::Error::other("controlled"))),
            Outcome::Success => file.try_lock(),
        }
    }
    fn wait(&self, duration: Duration, cancellation: &Cancellation) {
        self.waits.lock().unwrap().push(duration);
        if self.cancel_wait {
            cancellation.cancel();
        }
    }
}
fn controlled(outcomes: Vec<Outcome>, cancel_wait: bool) -> Arc<Controlled> {
    Arc::new(Controlled {
        outcomes: Mutex::new(outcomes.into()),
        attempts: 0.into(),
        waits: Default::default(),
        cancel_wait,
    })
}

#[test]
fn file_contention_uses_ten_attempts_with_twenty_ms_spacing() {
    for (outcomes, expected, attempts, waits) in [
        (
            vec![Outcome::Busy; 10],
            Err(CredentialError::Contended),
            10,
            9,
        ),
        (vec![Outcome::Success], Ok(()), 1, 0),
        (
            vec![Outcome::Busy, Outcome::Busy, Outcome::Success],
            Ok(()),
            3,
            2,
        ),
        (vec![Outcome::Failed], Err(CredentialError::Storage), 1, 0),
    ] {
        let scratch = Scratch::new();
        let mut storage = FileCredentialStorage::new(scratch.0.join("credentials.json")).unwrap();
        let hooks = controlled(outcomes, false);
        storage.acquisition = hooks.clone();
        let mut called = 0;
        assert_eq!(
            storage.transact(&Cancellation::new(), &mut |_| {
                called += 1;
                Ok(None)
            }),
            expected
        );
        assert_eq!(called, usize::from(expected.is_ok()));
        assert_eq!(hooks.attempts.load(Ordering::SeqCst), attempts);
        assert_eq!(
            *hooks.waits.lock().unwrap(),
            vec![Duration::from_millis(20); waits]
        );
    }
}

#[test]
fn file_wait_cancellation_skips_callback_and_preserves_owner() {
    let scratch = Scratch::new();
    let path = scratch.0.join("credentials.json");
    std::fs::write(&path, "{}").unwrap();
    let owner = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    owner.try_lock().unwrap();
    let mut storage = FileCredentialStorage::new(path.clone()).unwrap();
    let hooks = controlled(vec![Outcome::Busy], true);
    storage.acquisition = hooks.clone();
    let signal = Cancellation::new();
    let mut called = 0;
    assert_eq!(
        storage.transact(&signal, &mut |_| {
            called += 1;
            Ok(Some(SecretString::new("changed".into())))
        }),
        Err(CredentialError::Cancelled)
    );
    assert_eq!(called, 0);
    assert_eq!(hooks.attempts.load(Ordering::SeqCst), 1);
    let third = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    assert!(matches!(third.try_lock(), Err(TryLockError::WouldBlock)));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
    owner.unlock().unwrap();
    third.try_lock().unwrap();
    third.unlock().unwrap();
}
