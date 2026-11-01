//! The shell row's lifecycle, acknowledged through the real caller's phases.
use super::{
    Answer, Interception, World, active, intercepting_world, painted, poll_once, recorded, scripted,
};
use crate::{BashExecutionComponent, interactive_mode::tests::isolated};
use maestro_app::{BashError, BashExecOptions, BashExecResult, BashOperations};
use maestro_tui::tui::LocalFuture;
use std::{
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
};
use tokio::sync::oneshot;

/// A scripted shell: its chunks before and after the release, and where it reports its park.
struct Script {
    /// The chunks emitted before it waits for its release.
    before: Vec<&'static str>,
    /// The chunks emitted after the release.
    after: Vec<&'static str>,
    /// Reports that the script parked, for a script that waits at all.
    parked: Mutex<Option<oneshot::Sender<()>>>,
    /// The test's release, which the script waits for.
    release: Mutex<Option<oneshot::Receiver<()>>>,
}

impl BashOperations<maestro_app::Local> for Script {
    fn exec<'a>(
        &'a self,
        _command: &'a str,
        _cwd: &'a str,
        options: BashExecOptions<'a, dyn FnMut(&[u8]) + 'a>,
    ) -> Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + 'a>> {
        Box::pin(async move {
            let data = options.on_data;
            for chunk in &self.before {
                data(chunk.as_bytes());
            }
            let parked = self.parked.lock().unwrap().take();
            let release = self.release.lock().unwrap().take();
            if let (Some(parked), Some(release)) = (parked, release) {
                let _ = parked.send(());
                drop(release.await);
            }
            for chunk in &self.after {
                data(chunk.as_bytes());
            }
            Ok(BashExecResult {
                exit_code: Some(0.0),
            })
        })
    }
}

/// An answer supplying `script` as the command's operations.
fn scripting(script: &Rc<Script>) -> Answer {
    let operations = Rc::clone(script) as Rc<dyn BashOperations<maestro_app::Local>>;
    Rc::new(move |_event: &mut maestro_app::UserBashEvent| {
        Some(Interception {
            operations: Some(Rc::clone(&operations)),
            result: None,
        })
    })
}

/// The waker a caller registers, forwarded to the phase that waits for the caller's wake.
#[derive(Default)]
struct Signal {
    /// Whether the caller's waker was woken.
    woken: AtomicBool,
    /// The waiting future's waker.
    waiting: Mutex<Option<Waker>>,
}

impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.woken.store(true, Ordering::SeqCst);
        let waiting = self.waiting.lock().unwrap().take();
        if let Some(waker) = waiting {
            waker.wake();
        }
    }
}

/// Waits until the signal the caller's waker forwards is woken.
struct Notified(Arc<Signal>);

impl Future for Notified {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.0.woken.load(Ordering::SeqCst) {
            return Poll::Ready(());
        }
        *self.0.waiting.lock().unwrap() = Some(context.waker().clone());
        Poll::Pending
    }
}

/// One running shell command: its caller, the receiver it started, and its row.
struct Running {
    /// The mode the command runs in.
    world: World,
    /// The caller the mode started, as a submission does.
    caller: LocalFuture,
    /// The receiver the caller started for its chunks, held unrun.
    receiver: LocalFuture,
    /// The row the caller shows.
    row: Rc<BashExecutionComponent>,
}

/// Starts the mode's own shell command, holding the caller it returns.
fn calling(world: &World, command: &'static str) -> LocalFuture {
    let mode = Rc::clone(&world.mode);
    Box::pin(async move { mode.handle_bash_command(command, false).await })
}

impl Running {
    /// Polls the caller once and takes the receiver it started without running it.
    async fn start(
        world: World,
        caller: LocalFuture,
        parked: Option<oneshot::Receiver<()>>,
    ) -> Self {
        let mut caller = caller;
        let spawned = world.runtime.futures();
        assert!(
            poll_once(&mut caller).is_none(),
            "the caller waits for its command"
        );
        let mut futures = world.runtime.take_futures();
        assert_eq!(
            futures.len(),
            spawned + 1,
            "the caller started one receiver"
        );
        let receiver = futures.pop().expect("the caller started a receiver");
        drop(futures);
        if let Some(parked) = parked {
            assert!(parked.await.is_ok(), "the script parked");
        }
        let row = active(&world).expect("the caller shows its row");
        Self {
            world,
            caller,
            receiver,
            row,
        }
    }

    /// Applies the queued first chunk and leaves the receiver waiting.
    fn first_chunk_applied(&mut self) {
        let held = painted(&self.row);
        assert!(
            !held.contains("first"),
            "the held receiver paints nothing: {held}"
        );
        assert!(
            poll_once(&mut self.receiver).is_none(),
            "the receiver waits for the delivery to end"
        );
        let started = painted(&self.row);
        assert!(
            started.contains("first") && started.contains("Running..."),
            "the running row shows the first chunk: {started}"
        );
    }

    /// Releases the script, waits for its result, and checks the row lags the session.
    async fn release_and_record(&mut self, release: oneshot::Sender<()>) {
        let signal: Arc<Signal> = Arc::default();
        let waker = Waker::from(Arc::clone(&signal));
        let mut context = Context::from_waker(&waker);
        assert!(
            Future::poll(self.caller.as_mut(), &mut context).is_pending(),
            "the caller waits for the session"
        );
        let _ = release.send(());
        Notified(signal).await;
        let records = recorded(&self.world);
        assert_eq!(records.len(), 1, "the session recorded the command");
        assert_eq!(records[0]["output"], "first\nlast\n");
        let ready = painted(&self.row);
        assert!(
            !ready.contains("last"),
            "the held receiver has not painted the last chunk: {ready}"
        );
    }

    /// Waits for the drain, which must end before the caller completes.
    async fn drained(&mut self) {
        assert!(
            poll_once(&mut self.caller).is_none(),
            "caller must await drain, not complete"
        );
        assert!(
            !painted(&self.row).contains("last"),
            "the drain has not painted the last chunk: {}",
            painted(&self.row)
        );
        assert!(
            (&mut self.receiver).await.is_ok(),
            "the receiver applies its last chunk and ends"
        );
        let drained = painted(&self.row);
        assert!(
            drained.contains("last") && drained.contains("Running..."),
            "the drained row still runs: {drained}"
        );
    }

    /// Checks the caller completes after the drain and gives up its row claim.
    fn completed(&mut self) {
        assert!(
            poll_once(&mut self.caller).is_some_and(|outcome| outcome.is_ok()),
            "the caller completes after the drain"
        );
        let completed = painted(&self.row);
        assert!(
            !completed.contains("Running..."),
            "the completed row stops its loader: {completed}"
        );
        assert!(
            active(&self.world).is_none(),
            "the completed row gives up its claim"
        );
    }
}

#[test]
fn acknowledged_shell_drain_before_completion() {
    isolated(
        "acknowledged_shell_drain_before_completion",
        async |directory| {
            let (parked, parked_reached) = oneshot::channel();
            let (release, released) = oneshot::channel();
            let script = Rc::new(Script {
                before: vec!["first\n"],
                after: vec!["last\n"],
                parked: Mutex::new(Some(parked)),
                release: Mutex::new(Some(released)),
            });
            let shell = scripted(directory, "shell", "printf 'unused\\n'");
            let (world, _seen) = intercepting_world(directory, &shell, scripting(&script)).await;
            let caller = calling(&world, "echo lifecycle");
            async {
                let mut running = Running::start(world, caller, Some(parked_reached)).await;
                running.first_chunk_applied();
                running.release_and_record(release).await;
                running.drained().await;
                running.completed();
            }
            .await;
        },
    );
}

#[test]
fn acknowledged_shell_pending_teardown_releases_row() {
    isolated(
        "acknowledged_shell_pending_teardown_releases_row",
        async |directory| {
            let script = Rc::new(Script {
                before: vec!["first\n", "last\n"],
                after: Vec::new(),
                parked: Mutex::default(),
                release: Mutex::default(),
            });
            let shell = scripted(directory, "shell", "printf 'unused\\n'");
            let (world, _seen) = intercepting_world(directory, &shell, scripting(&script)).await;
            let caller = calling(&world, "echo teardown");
            async {
                // Both chunks stay queued in the unrun receiver while the caller waits.
                let running = Running::start(world, caller, None).await;
                assert_eq!(
                    recorded(&running.world).len(),
                    1,
                    "the session finished the command"
                );
                let Running {
                    world,
                    caller,
                    receiver,
                    row,
                } = running;
                let released = Rc::downgrade(&row);
                drop(row);
                drop(caller);
                drop(world);
                assert!(
                    receiver.await.is_ok(),
                    "the receiver applies its last chunk and ends"
                );
                assert!(
                    released.upgrade().is_none(),
                    "receiver termination releases actual torn-down row"
                );
            }
            .await;
        },
    );
}
