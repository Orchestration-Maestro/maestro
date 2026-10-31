//! Cleanup through the interactive presentation's public stop operation.

mod native;
mod ports;
use super::extension_ui::{RecordingWatcher, with_custom_theme};
use super::{Setup, World, isolated};
use crate::InteractiveMode;
use maestro_app::presentation_data::footer_data_provider::FooterOperations;
use maestro_tui::tui::InputHandler;
use maestro_tui::{Component, EditorComponent};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::{Rc, Weak},
};

/// A mode with independently observed repository and shared-theme registrations.
fn watched(directory: &Path, progress: bool) -> (World, Rc<RecordingWatcher>) {
    std::fs::create_dir_all(directory.join(".git")).unwrap();
    std::fs::write(directory.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let watcher = Rc::new(RecordingWatcher::default());
    let world = Setup {
        directories: Some(with_custom_theme(directory)),
        watcher: Some(Rc::clone(&watcher) as Rc<dyn FooterOperations>),
        configure: Box::new(move |settings| {
            settings.set_theme("custom".to_owned());
            settings.set_show_terminal_progress(progress);
        }),
        ..Setup::default()
    }
    .build(directory);
    assert_eq!(watcher.paths().len(), 2);
    assert_eq!(Path::new(&watcher.paths()[0]).file_name().unwrap(), ".git");
    assert!(watcher.paths()[1].ends_with("custom"));
    world.host.set_before_session_invalidate(Some(Rc::new(
        || -> Result<(), maestro_app::EventError> {
            panic!("presentation stop must not dispose the application runtime");
        },
    )));
    (world, watcher)
}

/// A removed subscription's capture that checks release outside retained guards.
struct Release {
    /// The mode must be observed weakly to avoid retaining its own subscription.
    mode: Weak<InteractiveMode>,
    /// Records destruction before UI stop.
    released: Rc<Cell<bool>>,
}

impl Drop for Release {
    fn drop(&mut self) {
        let mode = self.mode.upgrade().unwrap();
        assert!(mode.unsubscribe.try_borrow_mut().is_ok());
        assert!(mode.working.loading_animation.try_borrow_mut().is_ok());
        self.released.set(true);
    }
}

#[test]
fn stop_releases_loader_and_subscription_before_ui() {
    isolated(
        "stop_releases_loader_and_subscription_before_ui",
        async |directory| {
            for present in [false, true] {
                subscription_case(directory, present).await;
            }
        },
    );
}

#[test]
fn stop_failure_keeps_the_original_phase() {
    isolated("stop_failure_keeps_the_original_phase", async |directory| {
        for progress_failure in [true, false] {
            failure_case(directory, progress_failure).await;
        }
    });
}

#[test]
fn stop_unregisters_and_releases_only_current_owners() {
    isolated(
        "stop_unregisters_and_releases_only_current_owners",
        async |directory| {
            for initialized in [false, true] {
                owners_case(directory, initialized).await;
            }
        },
    );
}

/// Checks present or absent working owners and the subscription release boundary.
async fn subscription_case(directory: &Path, present: bool) {
    let (world, watcher) = watched(directory, false);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    let removed = Rc::new(Cell::new(false));
    let released = Rc::new(Cell::new(false));
    let unsubscribe = world.mode.unsubscribe.take().unwrap();
    if present {
        world.mode.show_working_loader();
        assert!(world.runtime.pending() > 0);
        let mode = Rc::downgrade(&world.mode);
        let registrations = Rc::clone(&watcher);
        let called = Rc::clone(&removed);
        let release = Release {
            mode: mode.clone(),
            released: Rc::clone(&released),
        };
        world.mode.unsubscribe.replace(Some(Box::new(move || {
            let _ = &release;
            let mode = mode.upgrade().unwrap();
            assert!(mode.working.loading_animation.borrow().is_none());
            assert_eq!(mode.status_container.children().borrow().len(), 1);
            assert!(registrations.closed(0), "provider precedes unsubscribe");
            assert!(mode.is_initialized.get(), "unsubscribe precedes UI stop");
            called.set(true);
            unsubscribe();
        })));
    } else {
        unsubscribe();
    }
    let stopped = Rc::clone(&released);
    world.terminal.set_tap(move |effect| {
        if effect == "\x1b[?25h" {
            assert_eq!(stopped.get(), present, "capture released before UI stop");
        }
    });
    let status = world.mode.status_container.render(80);
    world.mode.stop().unwrap();
    assert_eq!(removed.get(), present);
    assert_eq!(released.get(), present);
    assert!(world.mode.working.loading_animation.borrow().is_none());
    assert!(world.mode.unsubscribe.borrow().is_none());
    assert_eq!(world.mode.status_container.render(80), status);
    assert_eq!(world.runtime.pending(), 0, "no loader tick remains");
    world.terminal.clear_writes();
    world.host.session().set_session_name("after stop").unwrap();
    assert_eq!(
        world.runtime.pending(),
        0,
        "real session event is unsubscribed"
    );
    assert!(world.terminal.writes().is_empty());
    assert!(!world.mode.is_initialized.get());
}

/// Checks each failing phase through two attempts and a successful retry.
async fn failure_case(directory: &Path, progress_failure: bool) {
    let (world, watcher) = watched(directory, true);
    world.mode.init().await.unwrap();
    world.runtime.settle().unwrap();
    world.mode.show_working_loader();
    let kind = if progress_failure {
        world
            .terminal
            .set_progress_error(Some(std::io::ErrorKind::PermissionDenied));
        std::io::ErrorKind::PermissionDenied
    } else {
        world
            .terminal
            .set_stop_error(Some(std::io::ErrorKind::ConnectionReset));
        std::io::ErrorKind::ConnectionReset
    };
    for _ in 0..2 {
        assert_eq!(world.mode.stop().unwrap_err().kind(), kind);
        assert!(
            world.mode.is_initialized.get(),
            "failed UI stop stays initialized"
        );
        assert_eq!(watcher.closed(0), !progress_failure);
        assert!(!watcher.closed(1));
        assert_eq!(
            world.mode.working.loading_animation.borrow().is_some(),
            progress_failure
        );
        assert_eq!(world.mode.unsubscribe.borrow().is_some(), progress_failure);
        assert_eq!(world.runtime.futures(), 0);
    }
    assert_eq!(
        world.terminal.events(),
        if progress_failure {
            vec!["start"]
        } else {
            vec!["start", "stop", "stop"]
        }
    );
    recover_stop(&world, &watcher);
}

/// Clears the injected failures and verifies the remaining cleanup completes.
fn recover_stop(world: &World, watcher: &RecordingWatcher) {
    world.terminal.set_progress_error(None);
    world.terminal.set_stop_error(None);
    world.mode.stop().unwrap();
    assert!(!world.mode.is_initialized.get());
    assert!(watcher.closed(0));
    assert!(world.mode.working.loading_animation.borrow().is_none());
    assert!(world.mode.unsubscribe.borrow().is_none());
    assert_eq!(world.runtime.pending(), 0);
}

/// Checks both progress policies with one initialized-state policy.
async fn owners_case(directory: &Path, initialized: bool) {
    for progress in [false, true] {
        let (world, watcher) = watched(directory, progress);
        if initialized {
            world.mode.init().await.unwrap();
            world.runtime.settle().unwrap();
        }
        let effects = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::clone(&effects);
        let registrations = Rc::clone(&watcher);
        world.terminal.set_tap(move |effect| {
            if effect == "\x1b]9;4;0;\x07" {
                assert!(
                    !registrations.closed(0),
                    "progress precedes provider disposal"
                );
                observed.borrow_mut().push("progress");
            } else if effect == "\x1b[?25h" {
                assert!(
                    registrations.closed(0),
                    "provider disposal precedes UI stop"
                );
                observed.borrow_mut().push("ui");
            }
        });
        world.mode.stop().unwrap();
        assert!(!world.mode.is_initialized.get());
        assert!(watcher.closed(0));
        assert!(!watcher.closed(1), "the shared theme watch stays live");
        let expected: Vec<_> = [progress.then_some("progress"), initialized.then_some("ui")]
            .into_iter()
            .flatten()
            .collect();
        assert_eq!(*effects.borrow(), expected);
        assert_eq!(
            world.terminal.events(),
            if initialized {
                vec!["start", "stop"]
            } else {
                vec![]
            }
        );
        world.terminal.set_tap(|_| {});
        world.mode.stop().unwrap();
        assert_eq!(
            world
                .terminal
                .events()
                .iter()
                .filter(|event| **event == "stop")
                .count(),
            usize::from(initialized)
        );
        assert_eq!(
            world.runtime.futures(),
            0,
            "stop admits no disposal or exit work"
        );
    }
}

#[test]
fn graceful_shutdown_is_once_only_and_preserves_effect_order() {
    isolated(
        "graceful_shutdown_is_once_only_and_preserves_effect_order",
        async |directory| {
            for initialized in [false, true] {
                for progress in [false, true] {
                    graceful_case(directory, (initialized, progress)).await;
                }
            }
        },
    );
}

/// Checks synchronous admission and both asynchronous waits through real disposal.
async fn graceful_case(directory: &Path, branches: (bool, bool)) {
    let (initialized, progress) = branches;
    let effects = Rc::new(Effects::default());
    let (release, gate) = tokio::sync::oneshot::channel::<()>();
    let extension = pending_extension(Rc::clone(&effects.events), gate);
    let world = Setup {
        lifecycle: effects.clone(),
        extension: Some(extension),
        configure: Box::new(move |settings| settings.set_show_terminal_progress(progress)),
        ..Setup::default()
    }
    .build(directory);
    if initialized {
        world.mode.init().await.unwrap();
        world.runtime.settle().unwrap();
    } else {
        world.mode.register_signal_handlers().unwrap();
    }
    world.terminal.hold_drain();
    trace_terminal(&world.terminal, &effects.events);
    let mut shutdown = world.mode.shutdown();
    assert_eq!(*effects.events.borrow(), ["register", "remove"]);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(shutdown.as_mut().poll(&mut cx).is_pending());
    assert!(matches!(
        world.mode.shutdown().as_mut().poll(&mut cx),
        std::task::Poll::Ready(Ok(()))
    ));
    assert_eq!(*effects.events.borrow(), ["register", "remove"]);
    world.terminal.release_drain();
    assert!(shutdown.as_mut().poll(&mut cx).is_pending());
    assert!(!world.mode.is_initialized.get());
    assert!(matches!(
        world.mode.shutdown().as_mut().poll(&mut cx),
        std::task::Poll::Ready(Ok(()))
    ));
    release.send(()).unwrap();
    assert!(matches!(
        shutdown.as_mut().poll(&mut cx),
        std::task::Poll::Ready(Ok(()))
    ));
    assert_eq!(*effects.events.borrow(), graceful_expected(branches));
}

/// Reconstructs a uniquely strong-owned mode with controlled native effects.
pub(super) fn replace_effects(
    mode: Rc<InteractiveMode>,
    effects: Rc<dyn super::super::lifecycle::LifecycleEffects>,
) -> Rc<InteractiveMode> {
    let mut mode = Rc::try_unwrap(mode).unwrap_or_else(|_| panic!("unique mode"));
    mode.lifecycle.effects = effects;
    let mode = Rc::new_cyclic(|this| {
        mode.this = this.clone();
        mode
    });
    super::super::extensions::bind_runtime_hooks(&mode);
    mode
}

/// Signal callbacks whose ownership the controlled native registration keeps.
type Signals = (Rc<dyn Fn()>, Rc<dyn Fn()>);

/// Observable process effects without touching the real process.
pub(super) struct Effects {
    /// Wall time the clear action observes.
    pub(super) now: Cell<i64>,
    /// Effects in invocation order.
    pub(super) events: Rc<RefCell<Vec<String>>>,
    /// The currently registered callbacks, shared with their remover.
    signals: Rc<RefCell<Option<Signals>>>,
    /// Makes registration fail before publishing callbacks.
    fail_registration: Cell<bool>,
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            now: Cell::new(10000),
            events: Rc::default(),
            signals: Rc::default(),
            fail_registration: Cell::new(false),
        }
    }
}

impl super::super::lifecycle::LifecycleEffects for Effects {
    fn now_ms(&self) -> i64 {
        self.now.get()
    }
    fn register_signals(
        &self,
        terminate: Rc<dyn Fn()>,
        hangup: Rc<dyn Fn()>,
    ) -> std::io::Result<Box<dyn Fn()>> {
        if self.fail_registration.get() {
            return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        }
        self.events.borrow_mut().push("register".into());
        assert!(self.signals.borrow().is_none());
        *self.signals.borrow_mut() = Some((terminate, hangup));
        let signals = Rc::clone(&self.signals);
        let events = Rc::clone(&self.events);
        Ok(Box::new(move || {
            let removed = signals.take();
            if removed.is_some() {
                events.borrow_mut().push("remove".into());
            }
            drop(removed);
        }))
    }
    fn kill_tracked_detached_children(&self) {
        self.events.borrow_mut().push("kill".into());
    }
    fn exit(&self, status: i32) {
        self.events.borrow_mut().push(format!("exit:{status}"));
    }
}

/// One mode over observed terminal, runtime and process ports.
struct Rig {
    /// Observes the current mode exactly when its owned drain is created.
    before_drain: ports::DrainObservation,
    /// The actual presentation and application runtime.
    world: World,
    /// Controlled process effects.
    effects: Rc<Effects>,
    /// Independent stdout and stderr failure producers.
    outputs: [ports::Output; 2],
    /// Failure selected for the owned drain.
    drain_error: Rc<Cell<Option<std::io::ErrorKind>>>,
}
impl Rig {
    /// Builds the normal constructor path with independently observed effects.
    fn new(directory: &Path, mut setup: Setup) -> Self {
        let terminal = super::RecordingTerminal::new(80, 24);
        let runtime = super::ManualRuntime::new();
        let effects = Rc::new(Effects::default());
        let outputs = [ports::Output::default(), ports::Output::default()];
        let drain_error = Rc::new(Cell::new(None));
        let before_drain = Rc::default();
        setup.lifecycle = effects.clone();
        setup.terminal = Some((
            Rc::new(RefCell::new(ports::TerminalPort {
                before_drain: Rc::clone(&before_drain),
                inner: Box::new(terminal.clone()),
                output: outputs[0].clone(),
                drain_error: Rc::clone(&drain_error),
                events: Rc::clone(&effects.events),
            })),
            Rc::new(ports::RuntimePort {
                inner: runtime.handle(),
                output: outputs[1].clone(),
            }),
        ));
        let mut world = setup.build(directory);
        world.terminal = terminal;
        world.runtime = runtime;
        Self {
            before_drain,
            world,
            effects,
            outputs,
            drain_error,
        }
    }
    /// Initializes and settles the startup frame.
    async fn start(&self) {
        self.world.mode.init().await.unwrap();
        self.world.runtime.settle().unwrap();
    }
}

#[test]
fn lifecycle_callbacks_release_the_mode() {
    isolated("lifecycle_callbacks_release_the_mode", async |directory| {
        let rig = Rig::new(directory, Setup::default());
        rig.start().await;
        let weak = Rc::downgrade(&rig.world.mode);
        let signals = rig.effects.signals.borrow().clone().unwrap();
        let terminal = rig.world.terminal.clone();
        let stdout = rig.outputs[0].clone();
        let stderr = rig.outputs[1].clone();
        drop(rig.world);
        assert!(weak.upgrade().is_none());
        assert!(!stdout.registered());
        assert!(!stderr.registered());
        signals.0();
        signals.1();
        stdout.emit(&std::io::Error::from(std::io::ErrorKind::BrokenPipe));
        terminal.send_input("\x04");
        assert_eq!(*rig.effects.events.borrow(), ["register", "remove"]);
        let rig = Rig::new(directory, Setup::default());
        rig.world.terminal.hold_drain();
        let weak = Rc::downgrade(&rig.world.mode);
        let mut shutdown = rig.world.mode.shutdown();
        drop(rig.world.mode);
        assert!(weak.upgrade().is_some());
        rig.world.terminal.release_drain();
        assert!(matches!(super::typed::poll(&mut shutdown), Some(Ok(()))));
        drop(shutdown);
        assert!(weak.upgrade().is_none());
    });
}

#[test]
fn signal_registration_replaces_and_cancels_live_handlers() {
    isolated(
        "signal_registration_replaces_and_cancels_live_handlers",
        async |directory| {
            let rig = Rig::new(directory, Setup::default());
            let events = Rc::clone(&rig.effects.events);
            rig.world.terminal.set_tap(move |_| {
                assert_eq!(
                    events.borrow().first().map(String::as_str),
                    Some("register")
                );
            });
            rig.start().await;
            rig.world.mode.init().await.unwrap();
            assert_eq!(*rig.effects.events.borrow(), ["register"]);
            rig.world.mode.register_signal_handlers().unwrap();
            assert_eq!(
                *rig.effects.events.borrow(),
                ["register", "remove", "register"]
            );
            rig.world.mode.stop().unwrap();
            assert!(rig.effects.signals.borrow().is_none());
            assert!(rig.outputs.iter().all(|output| !output.registered()));
            rig.start().await;
            let second = Rig::new(directory, Setup::default());
            second.start().await;
            rig.world.mode.stop().unwrap();
            assert!(second.effects.signals.borrow().is_some());
            let failed = Rig::new(directory, Setup::default());
            failed.effects.fail_registration.set(true);
            let error = failed.world.mode.init().await.unwrap_err();
            assert_eq!(
                error.downcast_ref::<std::io::Error>().unwrap().kind(),
                std::io::ErrorKind::PermissionDenied
            );
            assert!(failed.world.terminal.events().is_empty());
            assert!(failed.outputs.iter().all(|output| !output.registered()));
        },
    );
}

#[test]
fn ordinary_output_errors_keep_native_categories() {
    isolated(
        "ordinary_output_errors_keep_native_categories",
        async |directory| {
            let rig = Rig::new(directory, Setup::default());
            rig.start().await;
            for error in [
                std::io::Error::from_raw_os_error(22),
                std::io::Error::other("EIO"),
            ] {
                for output in &rig.outputs {
                    output.emit(&error);
                }
            }
            assert_eq!(*rig.effects.events.borrow(), ["register"]);
            assert!(rig.world.mode.is_initialized.get());
        },
    );
}

#[test]
fn dead_output_errors_exit_without_normal_cleanup() {
    for case in ["stdout", "stderr"] {
        native::run(
            "dead_output_errors_exit_without_normal_cleanup",
            case,
            "",
            None,
        );
    }
    isolated(
        "dead_output_errors_exit_without_normal_cleanup",
        async |directory| {
            for code in [
                rustix::io::Errno::IO,
                rustix::io::Errno::PIPE,
                rustix::io::Errno::NOTCONN,
            ] {
                for source in 0..2 {
                    let rig = Rig::new(directory, Setup::default());
                    rig.start().await;
                    rig.effects.events.borrow_mut().clear();
                    let error = std::io::Error::from_raw_os_error(code.raw_os_error());
                    rig.outputs[source].emit(&error);
                    assert_eq!(*rig.effects.events.borrow(), ["remove", "kill", "exit:129"]);
                    assert!(rig.world.mode.is_initialized.get());
                    assert_eq!(rig.world.terminal.events(), ["start"]);
                    assert!(rig.world.mode.unsubscribe.borrow().is_some());
                    assert!(rig.outputs.iter().all(|output| !output.registered()));
                }
            }
        },
    );
}

#[test]
fn graceful_failures_keep_phase_and_never_report_success() {
    isolated(
        "graceful_failures_keep_phase_and_never_report_success",
        async |directory| {
            for phase in ["drain", "progress", "ui", "dispose"] {
                failure_shutdown(directory, phase).await;
            }
        },
    );
}

/// Checks the first failing effect's original category and the closed later-effect set.
async fn failure_shutdown(directory: &Path, phase: &str) {
    let rig = Rig::new(
        directory,
        Setup {
            extension: (phase == "dispose").then(failing_extension),
            configure: Box::new(|settings| settings.set_show_terminal_progress(true)),
            ..Setup::default()
        },
    );
    rig.start().await;
    let invalidated = Rc::new(Cell::new(false));
    let seen = Rc::clone(&invalidated);
    rig.world
        .host
        .set_before_session_invalidate(Some(Rc::new(move || {
            seen.set(true);
            Ok(())
        })));
    select_failure(&rig, phase);
    let mut shutdown = rig.world.mode.shutdown();
    let error = super::typed::poll(&mut shutdown)
        .expect("all selected effects settle immediately")
        .unwrap_err();
    if phase == "dispose" {
        assert!(
            error
                .downcast_ref::<maestro_app::DiagnosticErrorInfo>()
                .is_some()
        );
        assert!(!rig.world.mode.is_initialized.get());
    } else {
        let expected = match phase {
            "drain" => std::io::ErrorKind::ConnectionReset,
            "progress" => std::io::ErrorKind::PermissionDenied,
            _ => std::io::ErrorKind::BrokenPipe,
        };
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            expected
        );
        assert!(rig.world.mode.is_initialized.get());
    }
    assert!(!invalidated.get());
    assert_eq!(
        *rig.effects.events.borrow(),
        ["register", "remove", "drain"]
    );
    let mut duplicate = rig.world.mode.shutdown();
    assert!(matches!(super::typed::poll(&mut duplicate), Some(Ok(()))));
    assert_eq!(
        *rig.effects.events.borrow(),
        ["register", "remove", "drain"]
    );
}

#[test]
fn requested_shutdown_runs_after_agent_end_cleanup() {
    isolated(
        "requested_shutdown_runs_after_agent_end_cleanup",
        async |directory| {
            for requested in [false, true] {
                for transient in [(false, false), (false, true), (true, false), (true, true)] {
                    requested_case(directory, requested, transient).await;
                }
            }
        },
    );
}

/// Checks transient cleanup before consuming a requested shutdown, retaining transcript rows.
async fn requested_case(directory: &Path, requested: bool, transient: (bool, bool)) {
    let rig = Rig::new(directory, Setup::default());
    rig.start().await;
    let mode = &rig.world.mode;
    let completed: Rc<dyn Component> =
        Rc::new(maestro_tui::Text::new("completed".into(), 0, 0, None));
    mode.chat_container.add_child(completed);
    if transient.0 {
        mode.show_working_loader();
    }
    if transient.1 {
        super::events::start(
            &rig.world,
            &serde_json::json!([{"type":"text","text":"unfinished"}]),
        );
    }
    super::events::stale(&rig.world, "pending");
    let observed = Rc::downgrade(mode);
    rig.before_drain.replace(Some(Rc::new(move || {
        let mode = observed.upgrade().unwrap();
        assert!(mode.working.loading_animation.borrow().is_none());
        assert!(mode.streaming.borrow().is_none());
        assert!(mode.pending_tools.borrow().values().next().is_none());
    })));
    mode.shutdown_requested.set(requested);
    let ended = maestro_app::AgentEvent::AgentEnd {
        messages: Vec::new(),
    };
    super::events::dispatch(&rig.world, ended);
    assert!(mode.working.loading_animation.borrow().is_none());
    assert!(mode.streaming.borrow().is_none());
    assert!(mode.pending_tools.borrow().values().next().is_none());
    assert!(
        mode.chat_container
            .render(80)
            .join("\n")
            .contains("completed")
    );
    if requested {
        super::events::dispatch(
            &rig.world,
            maestro_app::AgentEvent::AgentEnd {
                messages: Vec::new(),
            },
        );
    }
    requested_result(&rig, requested);
}

#[test]
fn termination_signal_unregisters_and_disposes_before_terminal_writes() {
    native::run(
        "termination_signal_unregisters_and_disposes_before_terminal_writes",
        "term",
        "TERM",
        None,
    );
}

#[test]
fn hangup_runs_the_graceful_shutdown_and_exits_normally() {
    native::run(
        "hangup_runs_the_graceful_shutdown_and_exits_normally",
        "hup",
        "HUP",
        None,
    );
}

#[test]
fn second_signal_uses_native_default_during_pending_disposal() {
    for second in ["TERM", "HUP"] {
        native::run(
            "second_signal_uses_native_default_during_pending_disposal",
            "second",
            "TERM",
            Some(second),
        );
    }
}

/// Holds real runtime disposal until the controlled channel completes.
fn pending_extension(
    events: Rc<RefCell<Vec<String>>>,
    gate: tokio::sync::oneshot::Receiver<()>,
) -> maestro_app::ExtensionFactory<maestro_app::Local> {
    let gate = Rc::new(RefCell::new(Some(gate)));
    Rc::new(move |api| {
        let gate = Rc::clone(&gate);
        let events = Rc::clone(&events);
        api.on::<maestro_app::SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(move |_, _| {
                events.borrow_mut().push("dispose".into());
                let gate = gate.take().unwrap();
                Box::pin(async move {
                    gate.await.unwrap();
                    Ok(None)
                })
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    })
}

/// A real extension failure that the runtime's error listener can reject.
fn failing_extension() -> maestro_app::ExtensionFactory<maestro_app::Local> {
    Rc::new(|api| {
        api.on::<maestro_app::SessionShutdownEvent, ()>(
            "session_shutdown".into(),
            Rc::new(|_, _| {
                Box::pin(async {
                    Err(Box::new(std::io::Error::other("shutdown hook"))
                        as Box<dyn std::error::Error + Send + Sync>)
                })
            }),
        )
        .unwrap();
        Box::pin(async { Ok(()) })
    })
}

/// Selects one failing effect without changing any preceding phase.
fn select_failure(rig: &Rig, phase: &str) {
    if phase == "dispose" {
        drop(
            rig.world
                .host
                .session()
                .extension_runner()
                .on_error(Rc::new(|_| {
                    Err(Box::new(maestro_app::DiagnosticErrorInfo::new(
                        None,
                        "disposal rejected",
                    )))
                })),
        );
    }
    match phase {
        "drain" => rig
            .drain_error
            .set(Some(std::io::ErrorKind::ConnectionReset)),
        "progress" => rig
            .world
            .terminal
            .set_progress_error(Some(std::io::ErrorKind::PermissionDenied)),
        "ui" => rig
            .world
            .terminal
            .set_stop_error(Some(std::io::ErrorKind::BrokenPipe)),
        "dispose" => {}
        _ => unreachable!(),
    }
}

/// Asserts the requested and normal continuations after transient cleanup.
fn requested_result(rig: &Rig, requested: bool) {
    let mut work = rig.world.runtime.take_futures();
    assert_eq!(work.len(), usize::from(requested));
    if requested {
        assert_eq!(
            *rig.effects.events.borrow(),
            ["register", "remove", "drain"]
        );
        assert!(matches!(super::typed::poll(&mut work[0]), Some(Ok(()))));
        assert_eq!(
            *rig.effects.events.borrow(),
            ["register", "remove", "drain", "exit:0"]
        );
        assert!(!rig.world.mode.is_initialized.get());
        rig.world.runtime.settle().unwrap();
        assert_eq!(rig.world.runtime.pending(), 0);
    } else {
        assert!(rig.world.runtime.pending() > 0);
        assert_eq!(*rig.effects.events.borrow(), ["register"]);
    }
}

/// Records native progress clearing and UI restoration in the shared operation trace.
fn trace_terminal(terminal: &super::RecordingTerminal, events: &Rc<RefCell<Vec<String>>>) {
    let events = Rc::clone(events);
    terminal.set_tap(move |bytes| {
        if bytes == "\x1b[?25h" {
            events.borrow_mut().push("stop".into());
        }
        if bytes == "\x1b]9;4;0;\x07" {
            events.borrow_mut().push("progress".into());
        }
    });
}

/// The reference's complete current-owner effect order for each presentation policy.
fn graceful_expected(branches: (bool, bool)) -> Vec<&'static str> {
    match branches {
        (false, false) => vec!["register", "remove", "dispose", "exit:0"],
        (false, true) => vec!["register", "remove", "progress", "dispose", "exit:0"],
        (true, false) => vec!["register", "remove", "stop", "dispose", "exit:0"],
        (true, true) => vec![
            "register", "remove", "progress", "stop", "dispose", "exit:0",
        ],
    }
}

#[test]
fn clear_action_first_press_clears_and_redraws() {
    isolated(
        "clear_action_first_press_clears_and_redraws",
        async |directory| {
            let rig = Rig::new(directory, Setup::default());
            rig.start().await;
            rig.world.mode.editor.add_to_history("kept history");
            rig.world.mode.editor.set_text("unsubmitted");
            rig.world.runtime.settle().unwrap();
            rig.world.mode.editor.handle_input("\x03");
            assert_eq!(rig.world.mode.editor.get_text(), "");
            assert_eq!(rig.world.runtime.pending(), 1);
            assert_eq!(rig.world.mode.lifecycle.last_sigint_time.get(), 10000);
            assert_eq!(*rig.effects.events.borrow(), ["register"]);
            assert_eq!(
                super::typed::recalled(&rig.world.mode.editor),
                ["kept history"]
            );
            assert_eq!(rig.world.runtime.futures(), 0);
        },
    );
}

#[test]
fn clear_action_uses_strict_double_press_boundary() {
    isolated(
        "clear_action_uses_strict_double_press_boundary",
        async |directory| {
            for delta in [-1000, 0, 1, 498, 499, 500, 501, 1000] {
                let rig = Rig::new(directory, Setup::default());
                rig.start().await;
                rig.world.mode.editor.handle_input("\x03");
                rig.world.mode.editor.set_text("second draft");
                rig.world.runtime.settle().unwrap();
                rig.effects.now.set(10000 + delta);
                rig.world.mode.editor.handle_input("\x03");
                let exits = delta < 500;
                assert_eq!(
                    rig.world.runtime.futures(),
                    usize::from(exits),
                    "delta {delta}"
                );
                assert_eq!(
                    rig.world.mode.lifecycle.last_sigint_time.get(),
                    if exits { 10000 } else { 10000 + delta }
                );
                assert_eq!(
                    rig.world.mode.editor.get_text(),
                    if exits { "second draft" } else { "" }
                );
                finish_work(&rig.world);
                assert_eq!(
                    rig.effects
                        .events
                        .borrow()
                        .iter()
                        .filter(|effect| *effect == "exit:0")
                        .count(),
                    usize::from(exits)
                );
            }
        },
    );
}

/// Completes every admitted controlled operation and checks its real result.
fn finish_work(world: &World) {
    for mut future in world.runtime.take_futures() {
        assert!(matches!(super::typed::poll(&mut future), Some(Ok(()))));
    }
}

#[test]
fn remapped_clear_and_exit_actions_keep_empty_editor_admission() {
    isolated(
        "remapped_clear_and_exit_actions_keep_empty_editor_admission",
        async |directory| {
            for remapped in [false, true] {
                remapped_case(directory, remapped).await;
            }
        },
    );
}

/// Drives both registered actions through the editor's real empty-text interception.
async fn remapped_case(directory: &Path, remapped: bool) {
    let bindings = if remapped {
        serde_json::json!({"app.clear":"ctrl+y", "app.exit":"ctrl+x"})
    } else {
        serde_json::json!({})
    };
    std::fs::write(
        directory.join("keybindings.json"),
        serde_json::to_string_pretty(&bindings).unwrap(),
    )
    .unwrap();
    let rig = Rig::new(directory, Setup::default());
    rig.start().await;
    let clear = if remapped { "\x19" } else { "\x03" };
    let exit = if remapped { "\x18" } else { "\x04" };
    rig.world.mode.editor.set_text("clear this");
    rig.world.mode.editor.handle_input(clear);
    assert_eq!(rig.world.mode.editor.get_text(), "");
    rig.world.mode.editor.set_text("nonempty");
    rig.world.mode.editor.handle_input(exit);
    assert_eq!(
        rig.world.runtime.futures(),
        0,
        "nonempty editor refuses exit"
    );
    assert_eq!(*rig.effects.events.borrow(), ["register"]);
    rig.world.mode.editor.set_text("");
    rig.world.mode.editor.handle_input(exit);
    assert_eq!(rig.world.runtime.futures(), 1, "empty editor admits exit");
    assert_eq!(
        *rig.effects.events.borrow(),
        ["register", "remove", "drain"]
    );
    finish_work(&rig.world);
    assert_eq!(
        *rig.effects.events.borrow(),
        ["register", "remove", "drain", "exit:0"]
    );
}

/// The existing submit admission chosen by one literal input.
#[derive(Clone, Copy)]
enum QuitOutcome {
    /// Exact quit, after the shared whitespace trim.
    Quit,
    /// Ordinary input delivered and recorded as this text.
    Input(&'static str),
    /// Empty input does nothing.
    Empty,
}

#[test]
fn quit_command_exits_before_input_delivery_or_history() {
    isolated(
        "quit_command_exits_before_input_delivery_or_history",
        async |directory| {
            for (text, outcome) in [
                ("/quit", QuitOutcome::Quit),
                (" \t/quit\t ", QuitOutcome::Quit),
                ("\u{feff}/quit\u{feff}", QuitOutcome::Quit),
                ("/quit extra", QuitOutcome::Input("/quit extra")),
                ("/QUIT", QuitOutcome::Input("/QUIT")),
                ("/quit\u{85}", QuitOutcome::Input("/quit\u{85}")),
                ("", QuitOutcome::Empty),
                (" \t\u{feff}", QuitOutcome::Empty),
            ] {
                quit_case(directory, text, outcome).await;
            }
            for compacting in [false, true] {
                quit_busy_case(directory, compacting).await;
            }
        },
    );
}

/// Checks the actual installed submit handler, its waiter, history and frame boundary.
async fn quit_case(directory: &Path, text: &str, outcome: QuitOutcome) {
    let rig = Rig::new(directory, Setup::default());
    rig.start().await;
    rig.world.mode.editor.add_to_history("kept");
    rig.world.mode.editor.set_text("draft");
    rig.world.runtime.settle().unwrap();
    let mut waiter = super::typed::waiting(&rig.world.mode);
    super::typed::submit(&rig.world.mode, text);
    assert_quit_outcome(&rig, outcome, &mut waiter);
}

/// Checks quit settlement without mixing initialization into the outcome branches.
fn assert_quit_outcome(
    rig: &Rig,
    outcome: QuitOutcome,
    waiter: &mut std::pin::Pin<Box<dyn std::future::Future<Output = String>>>,
) {
    match outcome {
        QuitOutcome::Quit => {
            assert_eq!(rig.world.mode.editor.get_text(), "");
            assert_eq!(
                rig.world.runtime.pending(),
                0,
                "quit adds no clear-render frame"
            );
            assert_eq!(rig.world.runtime.futures(), 1);
            assert_eq!(
                *rig.effects.events.borrow(),
                ["register", "remove", "drain"]
            );
            assert!(super::typed::poll(waiter).is_none());
            finish_work(&rig.world);
            assert_eq!(super::typed::recalled(&rig.world.mode.editor), ["kept"]);
        }
        QuitOutcome::Input(expected) => {
            assert_eq!(super::typed::poll(waiter).as_deref(), Some(expected));
            assert_eq!(
                super::typed::recalled(&rig.world.mode.editor),
                [expected, "kept"]
            );
            assert_eq!(*rig.effects.events.borrow(), ["register"]);
        }
        QuitOutcome::Empty => {
            assert!(super::typed::poll(waiter).is_none());
            assert_eq!(rig.world.mode.editor.get_text(), "draft");
            assert_eq!(super::typed::recalled(&rig.world.mode.editor), ["kept"]);
            assert_eq!(*rig.effects.events.borrow(), ["register"]);
        }
    }
}

/// Keeps quit admission ahead of both real busy-session queue branches.
async fn quit_busy_case(directory: &Path, compacting: bool) {
    let rig = Rig::new(directory, Setup::default());
    rig.start().await;
    rig.world.mode.editor.add_to_history("kept");
    let mut waiter = super::typed::waiting(&rig.world.mode);
    let seen = Rc::new(Cell::new(false));
    let observed = Rc::clone(&seen);
    let submit = move |mode: &Rc<InteractiveMode>| {
        assert!(
            mode.runtime_host.session().is_streaming()
                || mode.runtime_host.session().is_compacting()
        );
        mode.editor.set_text("draft");
        super::typed::submit(mode, "/quit");
        assert_eq!(mode.editor.get_text(), "");
        assert!(mode.compaction_queued_messages.borrow().is_empty());
        assert!(
            mode.runtime_host
                .session()
                .get_steering_messages()
                .is_empty()
        );
        observed.set(true);
    };
    if compacting {
        super::queues::during_compaction(&rig.world, submit).await;
    } else {
        super::streaming(&rig.world, async |world, _| submit(&world.mode)).await;
    }
    assert!(seen.get());
    assert!(super::typed::poll(&mut waiter).is_none());
    assert_eq!(super::typed::recalled(&rig.world.mode.editor), ["kept"]);
    finish_work(&rig.world);
    assert_eq!(
        rig.effects
            .events
            .borrow()
            .iter()
            .filter(|event| *event == "exit:0")
            .count(),
        1
    );
    assert!(
        !rig.effects
            .events
            .borrow()
            .iter()
            .any(|event| event == "kill")
    );
}
