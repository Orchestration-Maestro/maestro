#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
//! The native host's clock, timers, local futures, environment and log files.
#[allow(dead_code, reason = "Each test crate uses part of the shared helpers.")]
mod runtime_support;

use std::error::Error;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use maestro_tui::tui::{LogContext, RenderCallback, TuiRuntime};
use maestro_tui_crossterm::ProcessTuiRuntime;
use runtime_support::{is_child, rerun, run_set};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::{LocalSet, yield_now};
use tokio::time::{Instant, advance, sleep};

/// Collects what the producers send until every sender is dropped, which closes the set.
async fn drained<T>(channel: &mut UnboundedReceiver<T>) -> Vec<T> {
    let mut values = Vec::new();
    while let Some(value) = channel.recv().await {
        values.push(value);
    }
    values
}

/// Whether nothing has been sent yet although a sender is still alive.
fn is_waiting<T>(receiver: &mut UnboundedReceiver<T>) -> bool {
    matches!(receiver.try_recv(), Err(TryRecvError::Empty))
}

/// A directory that is removed, ignoring failure, with everything below it when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// Creates the directory `name` below the system temporary directory.
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("maestro-runtime-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory created");
        Self(path)
    }

    /// A path below the directory.
    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An error with an optional cause that signals when it is dropped, after its report.
#[derive(Debug)]
struct Chain {
    message: &'static str,
    cause: Option<Box<Chain>>,
    reported: Option<UnboundedSender<()>>,
}

impl Chain {
    fn new(message: &'static str, cause: Option<Chain>) -> Self {
        Self {
            message,
            cause: cause.map(Box::new),
            reported: None,
        }
    }

    fn reported_to(mut self, sender: UnboundedSender<()>) -> Self {
        self.reported = Some(sender);
        self
    }
}

impl fmt::Display for Chain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message)
    }
}

impl Error for Chain {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

#[test]
fn runtime_clock_tracks_monotonic_elapsed_time() {
    run_set(true, |local| async move {
        let host = ProcessTuiRuntime::new(local);
        assert_eq!(host.now(), Duration::ZERO);
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(
            host.now(),
            Duration::ZERO,
            "wall time does not move the clock"
        );
        sleep(Duration::from_secs(5)).await;
        assert_eq!(host.now(), Duration::from_secs(5));
    });
}

#[test]
fn timers_defer_from_scheduling_and_detach_on_handle_drop() {
    run_set(true, |local| async move {
        let host = ProcessTuiRuntime::new(local);
        let origin = Instant::now();

        let (sender, mut fired) = unbounded_channel();
        let immediate = host.schedule(
            Duration::ZERO,
            Box::new(move || {
                sender.send(Instant::now() - origin).unwrap();
                Ok(())
            }),
        );
        assert!(is_waiting(&mut fired), "zero delay never runs inline");
        assert_eq!(drained(&mut fired).await, [Duration::ZERO]);
        drop(immediate);

        let (sender, mut fired) = unbounded_channel();
        let started = Instant::now();
        let detached = host.schedule(
            Duration::from_millis(100),
            Box::new(move || {
                sender.send(Instant::now() - started).unwrap();
                Ok(())
            }),
        );
        drop(detached);
        advance(Duration::from_millis(60)).await;
        assert!(is_waiting(&mut fired), "not before the deadline");
        assert_eq!(
            drained(&mut fired).await,
            [Duration::from_millis(100)],
            "the deadline counts from scheduling, and the callback ran once"
        );
    });
}

#[test]
fn cancelled_timers_drop_captures_without_firing() {
    run_set(true, |local| async move {
        let host = ProcessTuiRuntime::new(local);
        for (label, delay, polled_first) in [
            ("before polling", Duration::from_millis(50), false),
            ("while waiting", Duration::from_millis(50), true),
            ("zero delay before polling", Duration::ZERO, false),
        ] {
            let (sender, mut fired) = unbounded_channel();
            let mut timer = host.schedule(
                delay,
                Box::new(move || {
                    sender.send(()).unwrap();
                    Ok(())
                }),
            );
            if polled_first {
                yield_now().await;
                assert!(is_waiting(&mut fired), "{label}: still captured");
            }
            timer.cancel();
            timer.cancel();
            assert!(
                drained(&mut fired).await.is_empty(),
                "{label}: captures released without firing"
            );
        }

        let (sender, mut fired) = unbounded_channel();
        let mut done = host.schedule(
            Duration::ZERO,
            Box::new(move || {
                sender.send(()).unwrap();
                Ok(())
            }),
        );
        assert_eq!(drained(&mut fired).await, [()]);
        done.cancel();
        yield_now().await;
        assert!(
            drained(&mut fired).await.is_empty(),
            "cancelling after completion changes nothing"
        );
    });
}

/// Sends the entries a callback and its descendants record, in the order they run.
type Log = UnboundedSender<&'static str>;

/// A callback that spawns a future, which in turn schedules another callback.
fn reentrant_callback(host: Rc<ProcessTuiRuntime>, log: Log) -> RenderCallback {
    Box::new(move || {
        log.send("timer").unwrap();
        let nested = Rc::clone(&host);
        host.spawn_local(Box::pin(async move {
            log.send("future").unwrap();
            drop(nested.schedule(
                Duration::from_millis(5),
                Box::new(move || {
                    log.send("nested timer").unwrap();
                    Ok(())
                }),
            ));
            Ok(())
        }));
        Ok(())
    })
}

#[test]
fn local_futures_defer_and_allow_callback_reentry() {
    run_set(true, |local| async move {
        let host = Rc::new(ProcessTuiRuntime::new(local));

        let (sender, mut log) = unbounded_channel();
        let non_send = Rc::new(());
        host.spawn_local(Box::pin(async move {
            drop(non_send);
            sender.send("future").unwrap();
            Ok(())
        }));
        assert!(is_waiting(&mut log), "a future is never polled inline");
        assert_eq!(drained(&mut log).await, ["future"]);

        let (sender, mut log) = unbounded_channel();
        let _timer = host.schedule(Duration::ZERO, reentrant_callback(Rc::clone(&host), sender));
        assert_eq!(drained(&mut log).await, ["timer", "future", "nested timer"]);
    });
}

const EXPECTED_STDERR: &str =
    "timer failure\nretry later\ntimer cause\nfuture outer\nfuture middle\nfuture root\n";

/// Runs failing and successful work and checks that later work still completes.
fn report_scenario() {
    let panicked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&panicked);
    std::panic::set_hook(Box::new(move |_| {
        flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }));
    run_set(true, |local| async move {
        let host = ProcessTuiRuntime::new(local);
        let (sender, mut reported) = unbounded_channel();
        drop(host.schedule(
            Duration::ZERO,
            Box::new(move || {
                let cause = Chain::new("timer cause", None);
                let error = Chain::new("timer failure\nretry later", Some(cause));
                Err(io::Error::other(error.reported_to(sender)))
            }),
        ));
        drained(&mut reported).await;

        let (sender, mut reported) = unbounded_channel();
        host.spawn_local(Box::pin(async move {
            let root = Chain::new("future root", None);
            let middle = Chain::new("future middle", Some(root));
            let outer = Chain::new("future outer", Some(middle)).reported_to(sender);
            Err(Box::new(outer) as Box<dyn Error>)
        }));
        drained(&mut reported).await;

        let (sender, mut later) = unbounded_channel();
        let from_timer = sender.clone();
        drop(host.schedule(
            Duration::ZERO,
            Box::new(move || {
                from_timer.send(()).unwrap();
                Ok(())
            }),
        ));
        host.spawn_local(Box::pin(async move {
            sender.send(()).unwrap();
            Ok(())
        }));
        assert_eq!(
            drained(&mut later).await.len(),
            2,
            "work after a failure still completes"
        );
    });
    assert!(
        !panicked.load(std::sync::atomic::Ordering::SeqCst),
        "the host panicked"
    );
}

#[test]
fn failed_work_prints_each_source_and_keeps_running() {
    if is_child() {
        report_scenario();
        return;
    }
    let output = rerun("failed_work_prints_each_source_and_keeps_running", |_| {});
    assert_eq!(String::from_utf8_lossy(&output.stderr), EXPECTED_STDERR);
    #[cfg(unix)]
    {
        let (reader, writer) = rustix::pipe::pipe().expect("pipe created");
        drop(reader);
        let output = rerun(
            "failed_work_prints_each_source_and_keeps_running",
            |command| {
                command.stderr(std::process::Stdio::from(writer));
            },
        );
        assert!(output.stderr.is_empty());
    }
}

/// Records when the value it holds is dropped.
fn witness() -> (Rc<()>, Weak<()>) {
    let held = Rc::new(());
    let weak = Rc::downgrade(&held);
    (held, weak)
}

#[test]
fn dropping_local_set_releases_pending_work() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .expect("runtime builds");
    let local = Rc::new(LocalSet::new());
    let (timer_held, timer_released) = witness();
    let (future_held, future_released) = witness();
    runtime.block_on(local.run_until(async {
        let host = ProcessTuiRuntime::new(Rc::clone(&local));
        let timer = host.schedule(
            Duration::from_secs(3600),
            Box::new(move || {
                drop(timer_held);
                Ok(())
            }),
        );
        host.spawn_local(Box::pin(async move {
            std::future::pending::<()>().await;
            drop(future_held);
            Ok(())
        }));
        tokio::task::yield_now().await;
        drop(timer);
        drop(host);
    }));
    assert_eq!(
        Rc::strong_count(&local),
        1,
        "the host keeps no set handle after release"
    );
    assert!(timer_released.upgrade().is_some() && future_released.upgrade().is_some());
    drop(local);
    assert!(
        timer_released.upgrade().is_none(),
        "pending callback released"
    );
    assert!(
        future_released.upgrade().is_none(),
        "suspended future released"
    );
}

#[test]
fn environment_returns_values_without_normalization() {
    const VALUES: [(&str, &str); 7] = [
        ("MAESTRO_RUNTIME_EMPTY", ""),
        ("MAESTRO_RUNTIME_ONE", "1"),
        ("MAESTRO_RUNTIME_ZERO", "0"),
        ("MAESTRO_RUNTIME_TRUE", "true"),
        ("MAESTRO_RUNTIME_SPACES", "  padded  "),
        ("MAESTRO_RUNTIME_MARKS", "\u{feff}\u{85}"),
        ("MAESTRO_RUNTIME_UNICODE", "é日本🦀"),
    ];
    #[cfg(unix)]
    const BYTES: [(&str, &[u8], &str); 3] = [
        ("MAESTRO_RUNTIME_INVALID", b"A\xffB", "A\u{fffd}B"),
        ("MAESTRO_RUNTIME_TRUNCATED", b"\xe2\x82", "\u{fffd}"),
        (
            "MAESTRO_RUNTIME_SURROGATE",
            b"\xed\xa0\x80",
            "\u{fffd}\u{fffd}\u{fffd}",
        ),
    ];
    if is_child() {
        run_set(true, |local| async move {
            let host = ProcessTuiRuntime::new(local);
            for (key, value) in VALUES {
                assert_eq!(host.environment(key).as_deref(), Some(value), "{key}");
            }
            assert_eq!(host.environment("MAESTRO_RUNTIME_MISSING"), None);
            #[cfg(unix)]
            for (key, _, lossy) in BYTES {
                assert_eq!(host.environment(key).as_deref(), Some(lossy), "{key}");
            }
        });
        return;
    }
    rerun(
        "environment_returns_values_without_normalization",
        |command| {
            command.env_remove("MAESTRO_RUNTIME_MISSING");
            for (key, value) in VALUES {
                command.env(key, value);
            }
            #[cfg(unix)]
            for (key, bytes, _) in BYTES {
                use std::os::unix::ffi::OsStrExt;
                command.env(key, std::ffi::OsStr::from_bytes(bytes));
            }
        },
    );
}

/// Checks the clock, text and nonce of a fresh context against actual wall time.
fn checked_context(host: &ProcessTuiRuntime) -> LogContext {
    let now_ms = || {
        u64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        )
        .unwrap()
    };
    let before = now_ms();
    let context = host.log_context();
    let after = now_ms();
    assert!((before..=after).contains(&context.unix_ms));
    let instant =
        chrono::DateTime::from_timestamp_millis(i64::try_from(context.unix_ms).unwrap()).unwrap();
    assert_eq!(
        context.iso_time,
        instant.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    );
    assert!((1..=16).contains(&context.nonce.len()));
    assert!(
        context
            .nonce
            .chars()
            .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    );
    context
}

#[test]
fn log_context_uses_native_home_and_current_wall_time() {
    if is_child() {
        run_set(true, |local| async move {
            let context = checked_context(&ProcessTuiRuntime::new(local));
            match std::env::var_os("MAESTRO_EXPECT_HOME") {
                Some(home) => assert_eq!(context.home, Path::new(&home)),
                None => assert_eq!(context.home, std::env::home_dir().unwrap_or_default()),
            }
        });
        return;
    }
    run_set(true, |local| async move {
        let context = checked_context(&ProcessTuiRuntime::new(local));
        assert_eq!(context.home, std::env::home_dir().unwrap_or_default());
    });
    #[cfg(unix)]
    for home in [
        Some("/maestro-home-fixture"),
        Some("rel/home"),
        Some("/with space/home"),
        Some(""),
        None,
    ] {
        rerun(
            "log_context_uses_native_home_and_current_wall_time",
            |command| {
                command.env("TZ", "Pacific/Auckland");
                match home {
                    Some(home) => command.env("HOME", home),
                    None => command.env_remove("HOME"),
                };
                if let Some(exact) = home.filter(|home| !home.is_empty()) {
                    command.env("MAESTRO_EXPECT_HOME", exact);
                }
            },
        );
    }
}

/// A host for the file operations, which never use the set.
fn file_host() -> ProcessTuiRuntime {
    ProcessTuiRuntime::new(Rc::new(LocalSet::new()))
}

#[test]
fn append_log_preserves_bytes_without_creating_parents() {
    run_set(true, |_| async {
        let scratch = Scratch::new("append");
        let host = file_host();
        let missing = scratch.path("absent/out.log");
        let error = host.append_log(&missing, "x").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(
            !scratch.path("absent").exists(),
            "the parent is not created"
        );

        let file = scratch.path("out.log");
        host.append_log(&file, "first\r\n").unwrap();
        host.append_log(&file, "é\n").unwrap();
        assert_eq!(fs::read(&file).unwrap(), "first\r\né\n".as_bytes());

        host.append_log(&file, "").unwrap();
        assert_eq!(
            fs::read(&file).unwrap(),
            "first\r\né\n".as_bytes(),
            "an empty append keeps the seeded contents"
        );

        let empty = scratch.path("empty.log");
        host.append_log(&empty, "").unwrap();
        assert_eq!(fs::read(&empty).unwrap(), b"");
    });
}

#[test]
fn write_log_creates_parents_and_replaces_bytes() {
    run_set(true, |_| async {
        let scratch = Scratch::new("write");
        let host = file_host();
        let nested = scratch.path("a/b/out.log");
        host.write_log(&nested, "old content\r\n").unwrap();
        host.write_log(&nested, "new").unwrap();
        assert_eq!(fs::read(&nested).unwrap(), b"new");
        host.write_log(&nested, "").unwrap();
        assert_eq!(fs::read(&nested).unwrap(), b"");

        let bare = format!("maestro-runtime-bare-{}.log", std::process::id());
        host.write_log(Path::new(&bare), "bare").unwrap();
        let written = fs::read(&bare);
        let _ = fs::remove_file(&bare);
        assert_eq!(written.unwrap(), b"bare");
    });
}

/// Asserts that `error` is the operating system's `native` error for the same failure.
fn assert_native(error: &io::Error, native: &io::Error) {
    assert!(error.raw_os_error().is_some(), "{error:?} is the system's");
    assert_eq!(error.raw_os_error(), native.raw_os_error());
    assert_eq!(error.kind(), native.kind());
}

/// The error the operating system gives for appending to `path`.
fn native_append_error(path: &Path) -> io::Error {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_err()
}

#[test]
fn log_failures_keep_native_errors_and_failure_order() {
    run_set(true, |_| async {
        let scratch = Scratch::new("failures");
        let host = file_host();
        let directory = scratch.path("dir");
        fs::create_dir(&directory).unwrap();
        assert_native(
            &host.append_log(&directory, "x").unwrap_err(),
            &native_append_error(&directory),
        );
        assert_native(
            &host.write_log(&directory, "x").unwrap_err(),
            &fs::write(&directory, "x").unwrap_err(),
        );

        let blocker = scratch.path("blocker");
        fs::write(&blocker, "decoy").unwrap();
        let below = blocker.join("sub/out.log");
        assert_native(
            &host.write_log(&below, "x").unwrap_err(),
            &fs::create_dir_all(blocker.join("sub")).unwrap_err(),
        );
        assert_eq!(
            fs::read(&blocker).unwrap(),
            b"decoy",
            "the decoy is untouched"
        );
        let beneath = blocker.join("out.log");
        assert_native(
            &host.append_log(&beneath, "x").unwrap_err(),
            &native_append_error(&beneath),
        );
        assert_eq!(fs::read(&blocker).unwrap(), b"decoy");
    });
}

#[cfg(unix)]
#[test]
fn authored_log_paths_preserve_symlink_traversal() {
    run_set(true, |_| async {
        let scratch = Scratch::new("symlink");
        let host = file_host();
        fs::create_dir_all(scratch.path("real/sub")).unwrap();
        std::os::unix::fs::symlink(scratch.path("real/sub"), scratch.path("link")).unwrap();
        let through = scratch.path("link/../out.log");
        host.write_log(&through, "written").unwrap();
        assert_eq!(fs::read(scratch.path("real/out.log")).unwrap(), b"written");
        host.append_log(&through, "+appended").unwrap();
        assert_eq!(
            fs::read(scratch.path("real/out.log")).unwrap(),
            b"written+appended"
        );
        assert!(
            !scratch.path("out.log").exists(),
            "no lexically normalized decoy"
        );
    });
}

#[test]
fn real_driver_wakes_scheduled_work() {
    run_set(false, |local| async move {
        let host = ProcessTuiRuntime::new(local);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let _timer = host.schedule(
            Duration::from_millis(10),
            Box::new(move || {
                let _ = sender.send(());
                Ok(())
            }),
        );
        receiver.await.expect("the callback ran");
        assert!(host.now() >= Duration::from_millis(10));
    });
}
