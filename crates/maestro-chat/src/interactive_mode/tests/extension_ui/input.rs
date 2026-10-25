//! Terminal input through bound callbacks and the toolkit's focused editor.
use super::super::extension_widgets::with_factories;
use super::super::isolated;
use super::port;
use maestro_app::{ExtensionAPI, Local, TerminalInputHandler};
use maestro_tui::tui::InputListenerResult;
use std::{cell::Cell, rc::Rc};

#[test]
fn terminal_input_reuses_callback_identity_across_repeated_registration() {
    isolated(
        "terminal_input_reuses_callback_identity_across_repeated_registration",
        |directory| {
            let calls = Rc::new(Cell::new(0));
            let seen = calls.clone();
            let (world, mut handlers) =
                with_factories(directory, move |api| vec![counting(api, seen.clone())]);
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
        |directory| {
            for (decision, routed, edited) in [
                (0, vec!["raw", "after"], "rawafter"),
                (1, vec!["changed", "after"], "changedafter"),
                (2, vec!["", "after"], "after"),
                (3, vec!["after"], "after"),
            ] {
                let (world, mut handlers) =
                    with_factories(directory, move |api| vec![decided(api, decision)]);
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
        |directory| {
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
            });
            let remove = port(&world.mode)
                .on_terminal_input(handlers.pop().unwrap())
                .unwrap();
            let later = Rc::new(Cell::new(0));
            let calls = later.clone();
            let unrelated = world.mode.ui.add_input_listener(Rc::new(move |_| {
                calls.set(calls.get() + 1);
                InputListenerResult::Pass
            }));
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
        |directory| {
            let calls = Rc::new(Cell::new(0));
            let output = calls.clone();
            let (world, mut handlers) = with_factories(directory, move |api| {
                (0..2).map(|_| counting(api, output.clone())).collect()
            });
            let host = port(&world.mode);
            let first = host.on_terminal_input(handlers.pop().unwrap()).unwrap();
            let second = host.on_terminal_input(handlers.pop().unwrap()).unwrap();
            let unrelated_calls = Rc::new(Cell::new(0));
            let seen = unrelated_calls.clone();
            let unrelated = world.mode.ui.add_input_listener(Rc::new(move |_| {
                seen.set(seen.get() + 1);
                InputListenerResult::Pass
            }));
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
    isolated("terminal_input_cleanup_has_linear_work", |directory| {
        for count in [32, 64, 128] {
            let capture = Rc::new(Cell::new(0));
            let weak = Rc::downgrade(&capture);
            let (world, handlers) = with_factories(directory, move |api| {
                (0..count).map(|_| counting(api, capture.clone())).collect()
            });
            for handler in handlers {
                drop(port(&world.mode).on_terminal_input(handler).unwrap());
            }
            assert!(weak.upgrade().is_some());
            let calls = Rc::new(Cell::new(0));
            let seen = calls.clone();
            let unrelated = world.mode.ui.add_input_listener(Rc::new(move |_| {
                seen.set(seen.get() + 1);
                InputListenerResult::Pass
            }));
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
    });
}

/// A bound callback that counts the chunks reaching it.
fn counting(api: &ExtensionAPI<Local>, calls: Rc<Cell<usize>>) -> TerminalInputHandler {
    api.ui_callback(move |_, _| {
        calls.set(calls.get() + 1);
        Ok(InputListenerResult::Pass)
    })
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
        |directory| {
            let calls = Rc::new(Cell::new(0));
            let weak_capture = Rc::downgrade(&calls);
            let (world, mut handlers) =
                with_factories(directory, move |api| vec![counting(api, calls.clone())]);
            drop(
                port(&world.mode)
                    .on_terminal_input(handlers.pop().unwrap())
                    .unwrap(),
            );
            let terminal = world.terminal.clone();
            let ui = world.mode.ui.clone();
            let mode = Rc::downgrade(&world.mode);
            let seen = Rc::new(Cell::new(0));
            let output = seen.clone();
            let unrelated = ui.add_input_listener(Rc::new(move |_| {
                output.set(output.get() + 1);
                InputListenerResult::Pass
            }));
            drop(world);
            assert!(mode.upgrade().is_none());
            assert!(weak_capture.upgrade().is_none());
            terminal.send_input("kept");
            assert_eq!(seen.get(), 1);
            unrelated();
        },
    );
}
