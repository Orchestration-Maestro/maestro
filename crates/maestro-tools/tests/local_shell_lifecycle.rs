//! Local shell grace and completed observer lifetimes.
#![cfg(unix)]
#![cfg(test)]
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

#[test]
fn local_bash_timeout_fires_at_twenty_five_not_twenty_four_milliseconds() {
    runtime().block_on(async {
        let operations = create_local_bash_operations("/tools".into(), Some("/bin/bash".into()));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut sink = |data: &[u8]| {
            sender.send(data.to_vec()).unwrap();
        };
        let dir = TempDir::new();
        let gate = dir.0.join("alive");
        let command = format!(
            "printf ready; while [ ! -f '{}' ]; do :; done; printf alive; exec /bin/sleep 60",
            gate.display()
        );
        let mut operation = operations.exec(
            &command,
            "/",
            BashExecOptions {
                on_data: &mut sink,
                signal: None,
                timeout: Some(0.025),
                env: None,
            },
        );
        let waker = Waker::from(Arc::new(Wakes::default()));
        assert_eq!(
            next_chunk(&mut operation, &mut receiver, &waker).await,
            b"ready"
        );
        tokio::time::advance(Duration::from_millis(24)).await;
        assert_pending(&mut operation, &waker);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&gate)
            .unwrap();
        assert_eq!(
            next_chunk(&mut operation, &mut receiver, &waker).await,
            b"alive"
        );
        tokio::time::advance(Duration::from_millis(1)).await;
        let error = operation.await.unwrap_err();
        assert!(matches!(error, maestro_tools::BashError::Timeout(_)));
        assert_eq!(error.to_string(), "timeout:0.025");
    });
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
