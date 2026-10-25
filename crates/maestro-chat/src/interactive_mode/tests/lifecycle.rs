//! Cleanup through the interactive presentation's public stop operation.

mod native;
mod ports;
use super::extension_ui::{RecordingWatcher, with_custom_theme};
use super::{Setup, World, isolated};
use crate::InteractiveMode;
use maestro_app::presentation_data::footer_data_provider::FooterOperations;
use maestro_tui::Component;
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
    world.host.set_before_session_invalidate(Some(Rc::new(|| {
        panic!("presentation stop must not dispose the application runtime");
    })));
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
        |directory| {
            for present in [false, true] {
                subscription_case(directory, present);
            }
        },
    );
}

#[test]
fn stop_failure_keeps_the_original_phase() {
    isolated("stop_failure_keeps_the_original_phase", |directory| {
        for progress_failure in [true, false] {
            failure_case(directory, progress_failure);
        }
    });
}

#[test]
fn stop_unregisters_and_releases_only_current_owners() {
    isolated(
        "stop_unregisters_and_releases_only_current_owners",
        |directory| {
            for initialized in [false, true] {
                owners_case(directory, initialized);
            }
        },
    );
}

/// Checks present or absent working owners and the subscription release boundary.
fn subscription_case(directory: &Path, present: bool) {
    let (world, watcher) = watched(directory, false);
    world.mode.init().unwrap();
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
fn failure_case(directory: &Path, progress_failure: bool) {
    let (world, watcher) = watched(directory, true);
    world.mode.init().unwrap();
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
fn owners_case(directory: &Path, initialized: bool) {
    for progress in [false, true] {
        let (world, watcher) = watched(directory, progress);
        if initialized {
            world.mode.init().unwrap();
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
        |directory| {
            for initialized in [false, true] {
                for progress in [false, true] {
                    graceful_case(directory, (initialized, progress));
                }
            }
        },
    );
}

/// Checks synchronous admission and both asynchronous waits through real disposal.
fn graceful_case(directory: &Path, branches: (bool, bool)) {
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
        world.mode.init().unwrap();
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
    Rc::new_cyclic(|this| {
        mode.this = this.clone();
        mode
    })
}

/// Signal callbacks whose ownership the controlled native registration keeps.
type Signals = (Rc<dyn Fn()>, Rc<dyn Fn()>);

/// Observable process effects without touching the real process.
#[derive(Default)]
pub(super) struct Effects {
    /// Effects in invocation order.
    pub(super) events: Rc<RefCell<Vec<String>>>,
    /// The currently registered callbacks, shared with their remover.
    signals: Rc<RefCell<Option<Signals>>>,
    /// Makes registration fail before publishing callbacks.
    fail_registration: Cell<bool>,
}

impl super::super::lifecycle::LifecycleEffects for Effects {
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
        setup.lifecycle = effects.clone();
        setup.terminal = Some((
            Rc::new(RefCell::new(ports::TerminalPort {
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
            world,
            effects,
            outputs,
            drain_error,
        }
    }
    /// Initializes and settles the startup frame.
    fn start(&self) {
        self.world.mode.init().unwrap();
        self.world.runtime.settle().unwrap();
    }
}

#[test]
fn lifecycle_callbacks_release_the_mode() {
    isolated("lifecycle_callbacks_release_the_mode", |directory| {
        let rig = Rig::new(directory, Setup::default());
        rig.start();
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
        |directory| {
            let rig = Rig::new(directory, Setup::default());
            let events = Rc::clone(&rig.effects.events);
            rig.world.terminal.set_tap(move |_| {
                assert_eq!(
                    events.borrow().first().map(String::as_str),
                    Some("register")
                );
            });
            rig.start();
            rig.world.mode.init().unwrap();
            assert_eq!(*rig.effects.events.borrow(), ["register"]);
            rig.world.mode.register_signal_handlers().unwrap();
            assert_eq!(
                *rig.effects.events.borrow(),
                ["register", "remove", "register"]
            );
            rig.world.mode.stop().unwrap();
            assert!(rig.effects.signals.borrow().is_none());
            assert!(rig.outputs.iter().all(|output| !output.registered()));
            rig.start();
            let second = Rig::new(directory, Setup::default());
            second.start();
            rig.world.mode.stop().unwrap();
            assert!(second.effects.signals.borrow().is_some());
            let failed = Rig::new(directory, Setup::default());
            failed.effects.fail_registration.set(true);
            let error = failed.world.mode.init().unwrap_err();
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
        |directory| {
            let rig = Rig::new(directory, Setup::default());
            rig.start();
            for error in [
                std::io::Error::from_raw_os_error(22),
                std::io::Error::other("EIO"),
            ] {
                for output in &rig.outputs {
                    let kind = error.kind();
                    let errno = error.raw_os_error();
                    output.emit(&error);
                    assert_eq!(error.kind(), kind);
                    assert_eq!(error.raw_os_error(), errno);
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
        |directory| {
            for code in [
                rustix::io::Errno::IO,
                rustix::io::Errno::PIPE,
                rustix::io::Errno::NOTCONN,
            ] {
                for source in 0..2 {
                    let rig = Rig::new(directory, Setup::default());
                    rig.start();
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
        |directory| {
            for phase in ["drain", "progress", "ui", "dispose"] {
                failure_shutdown(directory, phase);
            }
        },
    );
}

/// Checks the first failing effect's original category and the closed later-effect set.
fn failure_shutdown(directory: &Path, phase: &str) {
    let rig = Rig::new(
        directory,
        Setup {
            extension: (phase == "dispose").then(failing_extension),
            configure: Box::new(|settings| settings.set_show_terminal_progress(true)),
            ..Setup::default()
        },
    );
    rig.start();
    let invalidated = Rc::new(Cell::new(false));
    let seen = Rc::clone(&invalidated);
    rig.world
        .host
        .set_before_session_invalidate(Some(Rc::new(move || seen.set(true))));
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
        |directory| {
            for requested in [false, true] {
                for transient in [(false, false), (false, true), (true, false), (true, true)] {
                    requested_case(directory, requested, transient);
                }
            }
        },
    );
}

/// Checks transient cleanup before consuming a requested shutdown, retaining transcript rows.
fn requested_case(directory: &Path, requested: bool, transient: (bool, bool)) {
    let rig = Rig::new(directory, Setup::default());
    rig.start();
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
    requested_result(&rig, requested);
}

#[test]
fn termination_signal_unregisters_and_drains_before_cleanup() {
    native::run(
        "termination_signal_unregisters_and_drains_before_cleanup",
        "term",
        "TERM",
        None,
    );
}

#[test]
fn hangup_skips_terminal_and_runtime_cleanup() {
    native::run(
        "hangup_skips_terminal_and_runtime_cleanup",
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
