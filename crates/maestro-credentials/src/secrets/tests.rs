use super::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};
struct Notify(std::thread::Thread);
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(Notify(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}
fn secret(value: &str) -> SecretString {
    SecretString::new(value.into())
}
struct Controlled {
    environment: Mutex<BTreeMap<String, String>>,
    commands: Mutex<Vec<(PathBuf, String)>>,
    output: Mutex<Vec<u8>>,
    status: bool,
    spawn_failure: bool,
    now: Arc<AtomicU64>,
    finishes_at: Option<u64>,
    kills: Arc<AtomicUsize>,
    finishes: Arc<AtomicUsize>,
    wait_gate: Option<(
        std::sync::mpsc::Sender<()>,
        Mutex<std::sync::mpsc::Receiver<()>>,
    )>,
    waits: Mutex<Vec<Duration>>,
    spawn_gate: Option<(
        std::sync::mpsc::Sender<()>,
        Mutex<std::sync::mpsc::Receiver<()>>,
    )>,
}
impl Controlled {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            environment: Default::default(),
            commands: Default::default(),
            output: Mutex::new(b"helper-value".to_vec()),
            status: true,
            spawn_failure: false,
            now: Arc::new(0.into()),
            finishes_at: Some(0),
            kills: Arc::new(0.into()),
            finishes: Arc::new(0.into()),
            spawn_gate: None,
            wait_gate: None,
            waits: Default::default(),
        })
    }
}
struct Process {
    runtime: Arc<Controlled>,
}
impl HelperProcess for Process {
    fn status(&mut self) -> Result<Option<bool>, ()> {
        Ok(self
            .runtime
            .finishes_at
            .filter(|end| self.runtime.now.load(Ordering::SeqCst) >= *end)
            .map(|_| self.runtime.status))
    }
    fn finish(self: Box<Self>, kill: bool) -> Option<Vec<u8>> {
        self.runtime.finishes.fetch_add(1, Ordering::SeqCst);
        if kill {
            self.runtime.kills.fetch_add(1, Ordering::SeqCst);
        }
        Some(self.runtime.output.lock().unwrap().clone())
    }
}
impl HelperRuntime for Arc<Controlled> {
    fn environment(&self, name: &str) -> Option<String> {
        self.environment.lock().unwrap().get(name).cloned()
    }
    fn spawn(
        &self,
        command: &str,
        directory: &std::path::Path,
    ) -> Result<Box<dyn HelperProcess>, ()> {
        self.commands
            .lock()
            .unwrap()
            .push((directory.into(), command.into()));
        if let Some((entered, release)) = &self.spawn_gate {
            entered.send(()).unwrap();
            release.lock().unwrap().recv().unwrap();
        }
        if self.spawn_failure {
            Err(())
        } else {
            Ok(Box::new(Process {
                runtime: self.clone(),
            }))
        }
    }
    fn now(&self) -> Duration {
        Duration::from_millis(self.now.load(Ordering::SeqCst))
    }
    fn wait(&self, duration: Duration) {
        self.waits.lock().unwrap().push(duration);
        if self.now.load(Ordering::SeqCst) + duration.as_millis() as u64 == 10_000
            && let Some((entered, release)) = &self.wait_gate
        {
            entered.send(()).unwrap();
            release.lock().unwrap().recv().unwrap();
        }
        self.now
            .fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
    }
}
fn resolver(runtime: Arc<Controlled>) -> NativeSecretResolver {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let mut resolver = NativeSecretResolver::new(PathBuf::from(format!(
        "/controlled/{}",
        NEXT.fetch_add(1, Ordering::SeqCst)
    )))
    .unwrap();
    resolver.runtime = Arc::new(runtime);
    resolver
}
fn resolved(resolver: &NativeSecretResolver, value: &str) -> Option<String> {
    block_on(resolver.resolve(secret(value), Cancellation::new()))
        .unwrap()
        .map(|s| s.expose().to_owned())
}
#[test]
fn configured_secret_uses_helper_environment_then_literal_rules() {
    let _guard = test_guard();
    let runtime = Controlled::new();
    runtime
        .environment
        .lock()
        .unwrap()
        .insert("EXACT".into(), "environment".into());
    runtime
        .environment
        .lock()
        .unwrap()
        .insert("EMPTY".into(), "".into());
    let resolver = resolver(runtime.clone());
    assert_eq!(
        resolved(&resolver, "!printf anything"),
        Some("helper-value".into())
    );
    assert_eq!(runtime.commands.lock().unwrap()[0].1, "printf anything");
    for (value, expected) in [
        ("EXACT", Some("environment")),
        ("EMPTY", Some("EMPTY")),
        ("missing", Some("missing")),
        ("a!b", Some("a!b")),
        ("${EXACT}", Some("${EXACT}")),
        ("", None),
    ] {
        assert_eq!(resolved(&resolver, value).as_deref(), expected);
    }
    runtime
        .environment
        .lock()
        .unwrap()
        .insert("EXACT".into(), "changed".into());
    assert_eq!(resolved(&resolver, "EXACT").as_deref(), Some("changed"));
    assert!(resolver.environment("EMPTY").is_none());
    assert!(resolver.environment("missing").is_none());
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
}

#[test]
fn helper_output_and_failure_are_resolved_without_leaking() {
    let _guard = test_guard();
    for (output, success, spawn_failure, expected) in [
        (
            b" \nfirst\nsecond\t\n".to_vec(),
            true,
            false,
            Some("first\nsecond"),
        ),
        (b" \n\t".to_vec(), true, false, None),
        (b"OUTPUT_SENTINEL".to_vec(), false, false, None),
        (vec![0xff], true, false, None),
        (b"OUTPUT_SENTINEL".to_vec(), true, true, None),
    ] {
        let mut runtime = Controlled::new();
        let owned = Arc::get_mut(&mut runtime).unwrap();
        owned.output = Mutex::new(output);
        owned.status = success;
        owned.spawn_failure = spawn_failure;
        let resolver = resolver(runtime.clone());
        let result =
            block_on(resolver.resolve(secret("!!COMMAND_SENTINEL"), Cancellation::new())).unwrap();
        assert_eq!(result.as_ref().map(|s| s.expose()), expected);
        assert!(!format!("{result:?}").contains("OUTPUT_SENTINEL"));
        assert!(!format!("{result:?}").contains("COMMAND_SENTINEL"));
        assert_eq!(runtime.commands.lock().unwrap()[0].1, "!COMMAND_SENTINEL");
        assert_eq!(
            runtime.finishes.load(Ordering::SeqCst),
            usize::from(!spawn_failure)
        );
    }
}

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: Mutex<()> = Mutex::new(());
    SERIAL.lock().unwrap_or_else(|p| p.into_inner())
}
fn poll_once<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(Notify(std::thread::current())));
    future.poll(&mut Context::from_waker(&waker))
}
fn gated_runtime() -> (
    Arc<Controlled>,
    std::sync::mpsc::Receiver<()>,
    std::sync::mpsc::Sender<()>,
) {
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut runtime = Controlled::new();
    Arc::get_mut(&mut runtime).unwrap().spawn_gate = Some((entered_tx, Mutex::new(release_rx)));
    (runtime, entered_rx, release_tx)
}
#[test]
fn helper_results_are_process_cached_until_explicit_reset() {
    let _guard = test_guard();
    reset_secret_helper_cache();
    for success in [true, false] {
        let mut runtime = Controlled::new();
        Arc::get_mut(&mut runtime).unwrap().status = success;
        let first = resolver(runtime.clone());
        let mut second = resolver(runtime.clone());
        second.working_directory = first.working_directory.clone();
        for _ in 0..2 {
            assert_eq!(resolved(&first, "!same").is_some(), success);
            assert_eq!(resolved(&second, "!same").is_some(), success);
        }
        assert_eq!(runtime.commands.lock().unwrap().len(), 1);
        resolved(&first, "!different");
        second.working_directory.push("other");
        resolved(&second, "!same");
        assert_eq!(runtime.commands.lock().unwrap().len(), 3);
        reset_secret_helper_cache();
        resolved(&first, "!same");
        assert_eq!(runtime.commands.lock().unwrap().len(), 4);
    }
    let (runtime, entered, release) = gated_runtime();
    let first = Arc::new(resolver(runtime.clone()));
    let (polled_tx, polled_rx) = std::sync::mpsc::channel();
    let mut threads = vec![];
    for _ in 0..2 {
        let resolver = first.clone();
        let polled = polled_tx.clone();
        threads.push(std::thread::spawn(move || {
            let mut future =
                std::pin::pin!(resolver.resolve(secret("!concurrent"), Cancellation::new()));
            assert!(poll_once(future.as_mut()).is_pending());
            polled.send(()).unwrap();
            block_on(future).unwrap().unwrap().expose().to_owned()
        }));
    }
    entered.recv().unwrap();
    polled_rx.recv().unwrap();
    polled_rx.recv().unwrap();
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
    release.send(()).unwrap();
    for thread in threads {
        assert_eq!(thread.join().unwrap(), "helper-value");
    }
    let (old_runtime, entered, release) = gated_runtime();
    *old_runtime.output.lock().unwrap() = b"old".to_vec();
    let old = Arc::new(resolver(old_runtime.clone()));
    let old_request = old.clone();
    let running = std::thread::spawn(move || resolved(&old_request, "!generation"));
    entered.recv().unwrap();
    reset_secret_helper_cache();
    let new_runtime = Controlled::new();
    *new_runtime.output.lock().unwrap() = b"new".to_vec();
    let mut new = resolver(new_runtime.clone());
    new.working_directory = old.working_directory.clone();
    assert_eq!(resolved(&new, "!generation").as_deref(), Some("new"));
    release.send(()).unwrap();
    assert_eq!(running.join().unwrap().as_deref(), Some("old"));
    assert_eq!(resolved(&old, "!generation").as_deref(), Some("new"));
    assert_eq!(old_runtime.commands.lock().unwrap().len(), 1);
    assert_eq!(new_runtime.commands.lock().unwrap().len(), 1);
}

#[test]
fn helper_timeout_is_ten_thousand_ms() {
    let _guard = test_guard();
    reset_secret_helper_cache();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut runtime = Controlled::new();
    let owned = Arc::get_mut(&mut runtime).unwrap();
    owned.finishes_at = None;
    owned.wait_gate = Some((entered_tx, Mutex::new(release_rx)));
    let resolver = Arc::new(resolver(runtime.clone()));
    let request = resolver.clone();
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    let thread =
        std::thread::spawn(move || result_tx.send(resolved(&request, "!timeout")).unwrap());
    entered_rx.recv().unwrap();
    assert_eq!(runtime.now.load(Ordering::SeqCst), 9980);
    assert!(matches!(
        result_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    assert_eq!(runtime.kills.load(Ordering::SeqCst), 0);
    release_tx.send(()).unwrap();
    assert!(result_rx.recv().unwrap().is_none());
    thread.join().unwrap();
    assert_eq!(runtime.now.load(Ordering::SeqCst), 10_000);
    assert_eq!(runtime.kills.load(Ordering::SeqCst), 1);
    assert_eq!(runtime.finishes.load(Ordering::SeqCst), 1);
    assert!(
        runtime
            .waits
            .lock()
            .unwrap()
            .iter()
            .all(|wait| *wait <= Duration::from_millis(20))
    );
    assert!(resolved(&resolver, "!timeout").is_none());
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
}

#[test]
fn cancelled_helper_waiter_does_not_cancel_other_waiters() {
    let _guard = test_guard();
    reset_secret_helper_cache();
    let (runtime, entered, release) = gated_runtime();
    let resolver = Arc::new(resolver(runtime.clone()));
    let first_signal = Cancellation::new();
    let signal = first_signal.clone();
    let first_resolver = resolver.clone();
    let (polled_tx, polled_rx) = std::sync::mpsc::channel();
    let first = std::thread::spawn(move || {
        let mut future = std::pin::pin!(first_resolver.resolve(secret("!shared"), signal));
        assert!(poll_once(future.as_mut()).is_pending());
        polled_tx.send(()).unwrap();
        block_on(future)
    });
    entered.recv().unwrap();
    polled_rx.recv().unwrap();
    let mut second = std::pin::pin!(resolver.resolve(secret("!shared"), Cancellation::new()));
    assert!(poll_once(second.as_mut()).is_pending());
    first_signal.cancel();
    assert_eq!(
        first.join().unwrap().err(),
        Some(CredentialError::Cancelled)
    );
    assert_eq!(runtime.kills.load(Ordering::SeqCst), 0);
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
    release.send(()).unwrap();
    assert_eq!(block_on(second).unwrap().unwrap().expose(), "helper-value");
    assert_eq!(
        resolved(&resolver, "!shared").as_deref(),
        Some("helper-value")
    );
    let pre_cancelled = Cancellation::new();
    pre_cancelled.cancel();
    assert_eq!(
        block_on(resolver.resolve(secret("!not-started"), pre_cancelled)).err(),
        Some(CredentialError::Cancelled)
    );
    assert_eq!(runtime.commands.lock().unwrap().len(), 1);
    assert_eq!(runtime.finishes.load(Ordering::SeqCst), 1);
}

#[cfg(unix)]
#[test]
fn exited_helper_with_inherited_stdout_times_out_without_partial_output() {
    let runtime = NativeRuntime {
        origin: Instant::now(),
    };
    let started = Instant::now();
    let result = helper_with_timeout(
        &runtime,
        "sleep 2 & printf token",
        &std::env::temp_dir(),
        Duration::from_millis(100),
    );
    assert!(result.is_none(), "stdout without EOF must be unavailable");
    assert!(started.elapsed() < Duration::from_millis(500));
}
