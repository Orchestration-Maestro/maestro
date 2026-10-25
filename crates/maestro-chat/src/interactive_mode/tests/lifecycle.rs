//! Cleanup through the interactive presentation's public stop operation.

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
