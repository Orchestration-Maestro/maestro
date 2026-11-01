//! Terminal input through bound callbacks and the toolkit's focused editor.
use super::super::extension_widgets::with_factories;
use super::super::isolated;
use super::port;
use maestro_app::{ExtensionAPI, Local, ReadonlyFooterDataProvider, TerminalInputHandler};
use maestro_tui::tui::{InputListener, InputListenerResult};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
};

#[test]
fn terminal_input_reuses_callback_identity_across_repeated_registration() {
    isolated(
        "terminal_input_reuses_callback_identity_across_repeated_registration",
        async |directory| {
            let calls = Rc::new(Cell::new(0));
            let seen = calls.clone();
            let (world, mut handlers) =
                with_factories(directory, move |api| vec![counting(api, seen.clone())]).await;
            let host = port(&world.mode);
            let handler = handlers.pop().unwrap();
            let first = host.on_terminal_input(handler.clone()).unwrap();
            let second = host.on_terminal_input(handler.clone()).unwrap();
            world.terminal.send_input("a");
            assert_eq!(calls.get(), 1);
            assert_eq!(world.mode.editor.get_text(), "a");
            second().unwrap();
            world.terminal.send_input("b");
            assert_eq!(calls.get(), 1);
            assert_eq!(world.mode.editor.get_text(), "ab");
            let current = host.on_terminal_input(handler).unwrap();
            world.terminal.send_input("c");
            assert_eq!(calls.get(), 2);
            first().unwrap();
            world.terminal.send_input("d");
            assert_eq!(
                calls.get(),
                2,
                "the old remover removes the current registration"
            );
            assert_eq!(world.mode.editor.get_text(), "abcd");
            current().unwrap();
            second().unwrap();
        },
    );
}

#[test]
fn terminal_input_pass_replace_and_consume_reach_the_focused_editor() {
    isolated(
        "terminal_input_pass_replace_and_consume_reach_the_focused_editor",
        async |directory| {
            for (decision, routed, edited) in [
                (0, vec!["raw", "after"], "rawafter"),
                (1, vec!["changed", "after"], "changedafter"),
                (2, vec!["", "after"], "after"),
                (3, vec!["after"], "after"),
            ] {
                let (world, mut handlers) =
                    with_factories(directory, move |api| vec![decided(api, decision)]).await;
                let remove = port(&world.mode)
                    .on_terminal_input(handlers.pop().unwrap())
                    .unwrap();
                let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
                let output = seen.clone();
                let unrelated = world.mode.ui.add_input_listener(Rc::new(move |data| {
                    output.borrow_mut().push(data.to_owned());
                    InputListenerResult::Pass
                }));
                world.terminal.send_input("raw");
                remove().unwrap();
                world.terminal.send_input("after");
                assert_eq!(*seen.borrow(), routed);
                assert_eq!(world.mode.editor.get_text(), edited);
                unrelated();
            }
        },
    );
}

#[test]
fn terminal_input_failure_reports_once_without_forwarding_the_chunk() {
    isolated(
        "terminal_input_failure_reports_once_without_forwarding_the_chunk",
        async |directory| {
            let address = Rc::new(Cell::new(0));
            let captured = address.clone();
            let (world, mut handlers) = with_factories(directory, move |api| {
                let captured = captured.clone();
                vec![api.ui_callback(move |_, _| {
                    let error: maestro_app::EventError =
                        Box::new(std::io::Error::from_raw_os_error(13));
                    captured.set(std::ptr::from_ref(&*error).cast::<()>().addr());
                    Err(error)
                })]
            })
            .await;
            let remove = port(&world.mode)
                .on_terminal_input(handlers.pop().unwrap())
                .unwrap();
            let later = Rc::new(Cell::new(0));
            let unrelated = world.mode.ui.add_input_listener(observing(later.clone()));
            world.terminal.send_input("refused");
            assert_eq!(later.get(), 0);
            assert_eq!(world.mode.editor.get_text(), "");
            let errors = world.runtime.drain_futures();
            assert_eq!(errors.len(), 1);
            assert_eq!(
                errors[0]
                    .downcast_ref::<std::io::Error>()
                    .unwrap()
                    .raw_os_error(),
                Some(13)
            );
            assert_eq!(
                std::ptr::from_ref(&*errors[0]).cast::<()>().addr(),
                address.get()
            );
            assert!(world.runtime.drain_futures().is_empty());
            remove().unwrap();
            world.terminal.send_input("after");
            assert_eq!(later.get(), 1);
            assert_eq!(world.mode.editor.get_text(), "after");
            unrelated();
        },
    );
}

#[test]
fn terminal_input_cleanup_removes_every_remaining_subscription() {
    isolated(
        "terminal_input_cleanup_removes_every_remaining_subscription",
        async |directory| {
            let calls = Rc::new(Cell::new(0));
            let output = calls.clone();
            let (world, mut handlers) = with_factories(directory, move |api| {
                (0..2).map(|_| counting(api, output.clone())).collect()
            })
            .await;
            let host = port(&world.mode);
            let first = host.on_terminal_input(handlers.pop().unwrap()).unwrap();
            let second = host.on_terminal_input(handlers.pop().unwrap()).unwrap();
            let unrelated_calls = Rc::new(Cell::new(0));
            let unrelated = world
                .mode
                .ui
                .add_input_listener(observing(unrelated_calls.clone()));
            first().unwrap();
            first().unwrap();
            world.terminal.send_input("a");
            assert_eq!(calls.get(), 1);
            world.mode.extension_input.clear();
            world.mode.extension_input.clear();
            second().unwrap();
            world.terminal.send_input("b");
            assert_eq!(calls.get(), 1);
            assert_eq!(unrelated_calls.get(), 2);
            assert_eq!(world.mode.editor.get_text(), "ab");
            unrelated();
        },
    );
}

#[test]
fn terminal_input_cleanup_has_linear_work() {
    isolated(
        "terminal_input_cleanup_has_linear_work",
        async |directory| {
            for count in [32, 64, 128] {
                let capture = Rc::new(Cell::new(0));
                let weak = Rc::downgrade(&capture);
                let (world, handlers) = with_factories(directory, move |api| {
                    (0..count).map(|_| counting(api, capture.clone())).collect()
                })
                .await;
                for handler in handlers {
                    drop(port(&world.mode).on_terminal_input(handler).unwrap());
                }
                assert!(weak.upgrade().is_some());
                let calls = Rc::new(Cell::new(0));
                let unrelated = world.mode.ui.add_input_listener(observing(calls.clone()));
                world.mode.extension_input.clear();
                assert!(
                    weak.upgrade().is_none(),
                    "cleanup releases every captured owner"
                );
                world.terminal.send_input("kept");
                assert_eq!(calls.get(), 1);
                assert_eq!(world.mode.editor.get_text(), "kept");
                unrelated();
            }
        },
    );
}

/// A bound callback that counts the chunks reaching it.
fn counting(api: &ExtensionAPI<Local>, calls: Rc<Cell<usize>>) -> TerminalInputHandler {
    let listener = observing(calls);
    api.ui_callback(move |data: String, _| Ok(listener(&data)))
}

/// A bound callback returning the selected input decision.
fn decided(api: &ExtensionAPI<Local>, decision: u8) -> TerminalInputHandler {
    api.ui_callback(move |_, _| {
        Ok(match decision {
            0 => InputListenerResult::Pass,
            1 => InputListenerResult::Replace("changed".to_owned()),
            2 => InputListenerResult::Replace(String::new()),
            _ => InputListenerResult::Consume,
        })
    })
}

#[test]
fn terminal_input_mode_drop_removes_only_extension_listeners() {
    isolated(
        "terminal_input_mode_drop_removes_only_extension_listeners",
        async |directory| {
            let calls = Rc::new(Cell::new(0));
            let weak_capture = Rc::downgrade(&calls);
            let (world, mut handlers) =
                with_factories(directory, move |api| vec![counting(api, calls.clone())]).await;
            drop(
                port(&world.mode)
                    .on_terminal_input(handlers.pop().unwrap())
                    .unwrap(),
            );
            let terminal = world.terminal.clone();
            let ui = world.mode.ui.clone();
            let mode = Rc::downgrade(&world.mode);
            let seen = Rc::new(Cell::new(0));
            let unrelated = ui.add_input_listener(observing(seen.clone()));
            drop(world);
            assert!(mode.upgrade().is_none());
            assert!(weak_capture.upgrade().is_none());
            terminal.send_input("kept");
            assert_eq!(seen.get(), 1);
            unrelated();
        },
    );
}

/// An ordinary toolkit listener that counts chunks without changing them.
fn observing(calls: Rc<Cell<usize>>) -> InputListener {
    Rc::new(move |_| {
        calls.set(calls.get() + 1);
        InputListenerResult::Pass
    })
}

/// Observes cleanup while the containing mode is still retained.
struct InputRelease {
    /// Filled after the callback factory constructs the mode.
    mode: std::cell::RefCell<std::rc::Weak<crate::InteractiveMode>>,
    /// The footer subscription must still be retained when input is released.
    footer: std::rc::Weak<Cell<usize>>,
    /// The UI must not yet have completed its stop.
    initialized: bool,
}

impl Drop for InputRelease {
    fn drop(&mut self) {
        let Some(mode) = self.mode.borrow().upgrade() else {
            return;
        };
        assert!(mode.working_loader().is_none());
        assert!(
            self.footer.upgrade().is_some(),
            "input precedes footer disposal"
        );
        assert_eq!(mode.is_initialized.get(), self.initialized);
        if self.initialized {
            assert!(
                mode.unsubscribe.borrow().is_some(),
                "input precedes unsubscribe"
            );
        }
    }
}

#[test]
fn stopping_the_mode_clears_only_extension_input_listeners() {
    isolated(
        "stopping_the_mode_clears_only_extension_input_listeners",
        async |directory| {
            for initialized in [false, true] {
                for count in [0, 3] {
                    stop_input_case(directory, initialized, count).await;
                }
            }
        },
    );
}

/// Retains the cleanup observation until the registered input callback is released.
fn retained_input(api: &ExtensionAPI<Local>, capture: &Rc<InputRelease>) -> TerminalInputHandler {
    let capture = capture.clone();
    api.ui_callback(move |_, _| {
        let _ = &capture;
        Ok(InputListenerResult::Pass)
    })
}

/// Checks capture release between the current loader and footer/UI owners.
async fn stop_input_case(directory: &Path, initialized: bool, count: usize) {
    let footer = Rc::new(Cell::new(0));
    let capture = Rc::new(InputRelease {
        mode: RefCell::default(),
        initialized,
        footer: Rc::downgrade(&footer),
    });
    let retained = Rc::downgrade(&capture);
    let (world, mut handlers) = with_factories(directory, move |api| {
        (0..count).map(|_| retained_input(api, &capture)).collect()
    })
    .await;
    if !initialized {
        world.mode.stop().unwrap();
    }
    if let Some(capture) = retained.upgrade() {
        capture.mode.replace(Rc::downgrade(&world.mode));
    }
    drop(
        world
            .mode
            .footer_data_provider
            .on_branch_change(Rc::new(move || {
                footer.set(footer.get() + 1);
            })),
    );
    let host = port(&world.mode);
    if let Some(handler) = handlers.pop() {
        let remove = host.on_terminal_input(handler.clone()).unwrap();
        let duplicate = host.on_terminal_input(handler).unwrap();
        remove().unwrap();
        duplicate().unwrap();
    }
    for handler in handlers {
        drop(host.on_terminal_input(handler.clone()).unwrap());
        drop(host.on_terminal_input(handler).unwrap());
    }
    let calls = Rc::new(Cell::new(0));
    let unrelated = world.mode.ui.add_input_listener(observing(calls.clone()));
    if initialized {
        world.mode.show_working_loader();
    }
    world.mode.stop().unwrap();
    assert!(
        retained.upgrade().is_none(),
        "stop releases extension captures"
    );
    world.mode.stop().unwrap();
    world.mode.init().await.unwrap();
    world.terminal.send_input("kept");
    assert_eq!(calls.get(), 1);
    assert_eq!(world.mode.editor.get_text(), "kept");
    unrelated();
}

#[test]
fn stopping_the_mode_keeps_input_clear_after_reinitialize() {
    isolated(
        "stopping_the_mode_keeps_input_clear_after_reinitialize",
        async |directory| {
            let calls = Rc::new(Cell::new(0));
            let seen = calls.clone();
            let (world, mut handlers) =
                with_factories(directory, move |api| vec![counting(api, seen.clone())]).await;
            let handler = handlers.pop().unwrap();
            let host = port(&world.mode);
            let old = host.on_terminal_input(handler.clone()).unwrap();
            let independent = Rc::new(Cell::new(0));
            let remove = world
                .mode
                .ui
                .add_input_listener(observing(independent.clone()));
            world.mode.stop().unwrap();
            world.mode.init().await.unwrap();
            world.terminal.send_input("a");
            assert_eq!(
                calls.get(),
                0,
                "stopped callback cannot return on reinitialize"
            );
            assert_eq!(independent.get(), 1, "unrelated listener survives");
            assert_eq!(world.mode.editor.get_text(), "a");
            let current = host.on_terminal_input(handler).unwrap();
            world.terminal.send_input("b");
            assert_eq!(calls.get(), 1);
            old().unwrap();
            world.terminal.send_input("c");
            assert_eq!(
                calls.get(),
                1,
                "old remover still removes a current registration"
            );
            assert_eq!(world.mode.editor.get_text(), "abc");
            assert_eq!(independent.get(), 3);
            current().unwrap();
            old().unwrap();
            remove();
            world.mode.stop().unwrap();
        },
    );
}

#[test]
fn stopping_the_mode_preserves_input_cleanup_failure_order() {
    isolated(
        "stopping_the_mode_preserves_input_cleanup_failure_order",
        async |directory| {
            for progress_failure in [true, false] {
                let capture = Rc::new(Cell::new(0));
                let retained = Rc::downgrade(&capture);
                let (world, mut handlers) =
                    with_factories(directory, move |api| vec![counting(api, capture.clone())])
                        .await;
                world
                    .host
                    .session()
                    .settings_manager()
                    .borrow_mut()
                    .set_show_terminal_progress(true);
                drop(
                    port(&world.mode)
                        .on_terminal_input(handlers.pop().unwrap())
                        .unwrap(),
                );
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
                for attempt in 1..=2 {
                    assert_eq!(world.mode.stop().unwrap_err().kind(), kind);
                    assert_eq!(
                        retained.upgrade().is_some(),
                        progress_failure,
                        "progress failure precedes input clear; UI failure follows it"
                    );
                    assert!(world.mode.is_initialized.get());
                    world.terminal.send_input("x");
                    assert_eq!(
                        retained.upgrade().map(|capture| capture.get()),
                        progress_failure.then_some(attempt)
                    );
                }
                world.terminal.set_progress_error(None);
                world.terminal.set_stop_error(None);
                world.mode.stop().unwrap();
                assert!(retained.upgrade().is_none());
                world.mode.init().await.unwrap();
                world.terminal.send_input("after");
                assert_eq!(
                    world.mode.editor.get_text(),
                    if progress_failure { "xxafter" } else { "after" }
                );
                world.mode.stop().unwrap();
            }
        },
    );
}
