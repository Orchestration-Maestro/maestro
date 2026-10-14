//! Local shell grace and completed observer lifetimes.
#![cfg(unix)]
#![cfg(test)]
use futures_util::FutureExt;
use maestro_models::Cancellation;
use maestro_tools::{BashExecOptions, create_local_bash_operations};
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[allow(
    dead_code,
    reason = "Shared polling helpers serve multiple integration targets."
)]
mod process_support;
mod read_support;
use read_support::TempDir;

/// Counts wakes without retaining the operation that registered them.
#[derive(Default)]
struct Wakes(AtomicUsize);
impl Wake for Wakes {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Runtime with an explicitly driven clock.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
}

#[test]
fn local_bash_unregisters_pid_and_releases_observers() {
    runtime().block_on(async {
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let signal = Cancellation::new();
        let counter = Arc::new(Wakes::default());
        let waker = Waker::from(counter.clone());
        let mut sink = |_: &[u8]| {};
        let mut operation = operations.exec(
            "exit 7",
            "/",
            BashExecOptions {
                on_data: &mut sink,
                signal: Some(&signal),
                timeout: Some(60.0),
                env: None,
            },
        );
        let result = loop {
            if let Poll::Ready(result) = operation.as_mut().poll(&mut Context::from_waker(&waker)) {
                break result;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(result.unwrap().exit_code, Some(7.0));
        let before = counter.0.load(Ordering::SeqCst);
        signal.abort();
        assert_eq!(
            counter.0.load(Ordering::SeqCst),
            before,
            "completed cancellation observer must not wake"
        );
        tokio::time::advance(Duration::from_secs(60)).await;
        assert_eq!(
            counter.0.load(Ordering::SeqCst),
            before,
            "completed timer must not wake"
        );
    });
}

/// Policy applied while a separately owned writer keeps the root's pipe open.
#[derive(Clone, Copy)]
enum GracePolicy {
    Stream,
    Eof,
    Abort,
    Timeout,
    TimeoutAbort,
    AbortTimeout,
}

#[test]
#[cfg(target_os = "linux")]
fn local_bash_retains_streaming_during_exit_grace() {
    runtime().block_on(async {
        grace_case(GracePolicy::Stream).await;
        grace_case(GracePolicy::Eof).await;
    });
}

#[test]
#[cfg(target_os = "linux")]
fn local_bash_orders_abort_and_timeout_during_exit_grace() {
    runtime().block_on(async {
        for policy in [
            GracePolicy::Abort,
            GracePolicy::Timeout,
            GracePolicy::TimeoutAbort,
            GracePolicy::AbortTimeout,
        ] {
            grace_case(policy).await;
        }
    });
}

/// Exercises one execution with an independently reaped writer holding its pipe.
#[cfg(target_os = "linux")]
async fn grace_case(policy: GracePolicy) {
    let dir = TempDir::new();
    let gate = dir.0.join("exit");
    let command = format!(
        "printf '%s\\n' \"$$\"; while [ ! -f '{}' ]; do :; done",
        gate.display()
    );
    let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
    let signal = Cancellation::new();
    let timeout = (!matches!(
        policy,
        GracePolicy::Stream | GracePolicy::Eof | GracePolicy::Abort
    ))
    .then_some(0.025);
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut sink = |data: &[u8]| {
        sender.send(data.to_vec()).unwrap();
    };
    let mut operation = operations.exec(
        &command,
        "/",
        BashExecOptions {
            on_data: &mut sink,
            signal: Some(&signal),
            timeout,
            env: None,
        },
    );
    let waker = Waker::from(Arc::new(Wakes::default()));
    let pid_bytes = next_chunk(&mut operation, &mut receiver, &waker).await;
    let pid = String::from_utf8(pid_bytes)
        .unwrap()
        .trim()
        .parse::<u32>()
        .unwrap();
    let mut holder = pipe_holder(pid).await;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&gate)
        .unwrap();
    while std::path::Path::new(&format!("/proc/{pid}")).exists() {
        assert_pending(&mut operation, &waker);
        tokio::task::yield_now().await;
    }
    assert_pending(&mut operation, &waker);
    apply_grace_policy(policy, &signal, &mut operation, &waker).await;
    holder
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"emit\n")
        .await
        .unwrap();
    assert_eq!(
        next_chunk(&mut operation, &mut receiver, &waker).await,
        b"grace"
    );
    finish_grace(policy, &mut operation, holder, &waker).await;
}

/// Settles one grace result and reaps the independent writer.
async fn finish_grace(
    policy: GracePolicy,
    operation: &mut Operation<'_>,
    mut holder: tokio::process::Child,
    waker: &Waker,
) {
    if matches!(policy, GracePolicy::Eof) {
        holder.stdin.take();
    } else {
        tokio::time::advance(Duration::from_millis(1)).await;
    }
    let result = loop {
        if let Poll::Ready(result) = operation.as_mut().poll(&mut Context::from_waker(waker)) {
            break result;
        }
        tokio::task::yield_now().await;
    };
    assert_grace_result(policy, result);
    if !matches!(policy, GracePolicy::Eof) {
        assert!(holder.try_wait().unwrap().is_none());
        holder.stdin.take();
    }
    assert!(holder.wait().await.unwrap().success());
}

/// Advances only the selected policy, leaving the final grace millisecond outstanding.
async fn apply_grace_policy(
    policy: GracePolicy,
    signal: &Cancellation,
    operation: &mut Operation<'_>,
    waker: &Waker,
) {
    if matches!(policy, GracePolicy::Abort | GracePolicy::AbortTimeout) {
        signal.abort();
    }
    assert_pending(operation, waker);
    if matches!(
        policy,
        GracePolicy::Timeout | GracePolicy::TimeoutAbort | GracePolicy::AbortTimeout
    ) {
        tokio::time::advance(Duration::from_millis(25)).await;
        assert_pending(operation, waker);
        if matches!(policy, GracePolicy::TimeoutAbort) {
            signal.abort();
        }
        assert_pending(operation, waker);
        tokio::time::advance(Duration::from_millis(74)).await;
    } else {
        let duration = match policy {
            GracePolicy::Eof => Duration::ZERO,
            _ => Duration::from_millis(99),
        };
        tokio::time::advance(duration).await;
    }
    assert_pending(operation, waker);
}

/// The replaceable operation's borrowed execution future.
type Operation<'a> = std::pin::Pin<
    Box<
        dyn std::future::Future<
                Output = Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
            > + Send
            + 'a,
    >,
>;

/// A separately reaped process holds the shell's stdout across root exit.
#[cfg(target_os = "linux")]
async fn pipe_holder(pid: u32) -> tokio::process::Child {
    let mut command = tokio::process::Command::new("/bin/bash");
    command.args(["-c", &format!("exec 3>/proc/{pid}/fd/1; printf held; read action; printf grace >&3; read finish; exit 0")]);
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().unwrap();
    let mut ready = [0; 4];
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut ready)
        .await
        .unwrap();
    assert_eq!(&ready, b"held");
    child
}

/// Drives the same operation until its sink delivers another chunk.
async fn next_chunk(
    operation: &mut Operation<'_>,
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    waker: &Waker,
) -> Vec<u8> {
    loop {
        assert_pending(operation, waker);
        if let Ok(chunk) = receiver.try_recv() {
            return chunk;
        }
        tokio::task::yield_now().await;
    }
}

/// Termination policy exercised against the gated native child.
#[derive(Clone, Copy)]
enum Stop {
    /// Abort from the complete readiness callback.
    Abort,
    /// Expire the operation timer after the alive acknowledgement.
    Timeout,
}

#[test]
fn local_bash_aborts_from_stream_callback() {
    runtime().block_on(termination_case(Stop::Abort));
}

#[test]
fn local_bash_timeout_fires_at_twenty_five_not_twenty_four_milliseconds() {
    runtime().block_on(termination_case(Stop::Timeout));
}

/// Observations retained while polling the same operation through cleanup.
#[derive(Default)]
struct ChildWitness {
    /// Unconsumed callback bytes, including any partial acknowledgement.
    bytes: Vec<u8>,
    /// Root PID reported by the native shell.
    pid: Option<u32>,
    /// Result of the same execution, captured at most once.
    settled: Option<Result<maestro_tools::BashExecResult, maestro_tools::BashError>>,
}

impl ChildWitness {
    /// Saves settlement so a completed future is never polled twice.
    async fn poll(&mut self, operation: &mut Operation<'_>) {
        if self.settled.is_none()
            && let Poll::Ready(result) =
                process_support::poll_once(&mut std::pin::Pin::new(operation)).await
        {
            self.settled = Some(result);
        }
    }

    /// Collects a complete acknowledgement while keeping the clock driver runnable.
    async fn line(
        &mut self,
        operation: &mut Operation<'_>,
        receiver: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Vec<u8> {
        loop {
            self.poll(operation).await;
            while let Ok(chunk) = receiver.try_recv() {
                self.bytes.extend(chunk);
            }
            if let Some(end) = self.bytes.iter().position(|byte| *byte == b'\n') {
                return self.bytes.drain(..=end).collect();
            }
            assert!(
                self.settled.is_none(),
                "child settled before acknowledgement"
            );
            tokio::task::yield_now().await;
        }
    }
}

/// Creates a gate without truncating an existing entry.
fn release(dir: &TempDir, name: &str) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.0.join(name))
}

/// Observes the 24ms acknowledgement and delivers the timer at exactly 25ms.
async fn drive_timeout(
    witness: &mut ChildWitness,
    operation: &mut Operation<'_>,
    receiver: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
    dir: &TempDir,
) {
    let start = tokio::time::Instant::now();
    tokio::time::advance(Duration::from_millis(24)).await;
    witness.poll(operation).await;
    assert!(witness.settled.is_none());
    release(dir, "alive").unwrap();
    assert_eq!(witness.line(operation, receiver).await, b"alive\n");
    tokio::time::advance(Duration::from_millis(1)).await;
    tokio::time::sleep(Duration::ZERO).await;
    assert_eq!(start.elapsed(), Duration::from_millis(25));
    witness.poll(operation).await;
}

/// Attempts both gate releases, terminating a known PID on errors other than an existing gate.
/// Then drives the same execution to settlement.
async fn settle_child(witness: &mut ChildWitness, operation: &mut Operation<'_>, dir: &TempDir) {
    for name in ["alive", "release"] {
        if let Err(error) = release(dir, name)
            && error.kind() != std::io::ErrorKind::AlreadyExists
            && let Some(pid) = witness.pid
        {
            maestro_tools::utils::shell::kill_process_tree(pid);
        }
    }
    while witness.settled.is_none() {
        witness.poll(operation).await;
        tokio::task::yield_now().await;
    }
}

/// Proves the selected stop effect before checking its final error.
async fn termination_case(stop: Stop) {
    let dir = TempDir::new();
    let cwd = dir.text();
    let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
    let signal = Cancellation::new();
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut readiness = Vec::new();
    let mut sink = |data: &[u8]| {
        readiness.extend_from_slice(data);
        if matches!(stop, Stop::Abort) && readiness.contains(&b'\n') {
            signal.abort();
        }
        let _ = sender.send(data.to_vec());
    };
    let mut operation = operations.exec(
        "printf '%s\\n' \"$$\"; while [ ! -f alive ] && [ ! -f release ]; do :; done; printf 'alive\\n'; while [ ! -f release ]; do :; done; printf survived > survived",
        &cwd,
        BashExecOptions {
            on_data: &mut sink,
            signal: Some(&signal),
            timeout: matches!(stop, Stop::Timeout).then_some(0.025),
            env: None,
        },
    );
    let mut witness = ChildWitness::default();
    let observation = std::panic::AssertUnwindSafe(async {
        let line = witness.line(&mut operation, &mut receiver).await;
        witness.pid = Some(std::str::from_utf8(&line).unwrap().trim().parse().unwrap());
        match stop {
            Stop::Abort => witness.poll(&mut operation).await,
            Stop::Timeout => drive_timeout(&mut witness, &mut operation, &mut receiver, &dir).await,
        }
    })
    .catch_unwind()
    .await;
    settle_child(&mut witness, &mut operation, &dir).await;
    if let Err(panic) = observation {
        std::panic::resume_unwind(panic);
    }
    assert_termination(stop, &signal, witness, &dir);
}

/// Checks the reaped root, closed marker observation set and exact error.
fn assert_termination(stop: Stop, signal: &Cancellation, witness: ChildWitness, dir: &TempDir) {
    #[cfg(target_os = "linux")]
    assert!(!std::path::Path::new(&format!("/proc/{}", witness.pid.unwrap())).exists());
    assert!(
        !dir.0.join("survived").exists(),
        "child survived the stop effect"
    );
    let error = witness.settled.unwrap().unwrap_err();
    match stop {
        Stop::Abort => {
            assert!(signal.is_aborted());
            assert!(matches!(error, maestro_tools::BashError::Aborted));
            assert_eq!(error.to_string(), "aborted");
        }
        Stop::Timeout => {
            assert!(matches!(error, maestro_tools::BashError::Timeout(_)));
            assert_eq!(error.to_string(), "timeout:0.025");
        }
    }
}

/// Polls the operation once and rejects premature settlement.
fn assert_pending(operation: &mut Operation<'_>, waker: &Waker) {
    assert!(
        operation
            .as_mut()
            .poll(&mut Context::from_waker(waker))
            .is_pending()
    );
}

/// Checks the final error ordering for the same completed operation.
fn assert_grace_result(
    policy: GracePolicy,
    result: Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
) {
    match policy {
        GracePolicy::Stream | GracePolicy::Eof => assert_eq!(result.unwrap().exit_code, Some(0.0)),
        GracePolicy::Timeout => {
            assert!(matches!(result, Err(maestro_tools::BashError::Timeout(_))));
        }
        _ => assert!(matches!(result, Err(maestro_tools::BashError::Aborted))),
    }
}
