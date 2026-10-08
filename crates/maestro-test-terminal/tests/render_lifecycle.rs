//! Lifecycle, defaults and render scheduling of the retained frame writer.

use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::images::terminal_image::{ImageProtocol, TerminalCapabilities};
use maestro_tui::tui::ComponentHandle;
use maestro_tui::{CURSOR_MARKER, Container, TUI, TerminalImage};

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod components;
    pub mod manual_runtime;
    pub mod recording_terminal;
}
use support::checks::succeeds;
use support::components::{Cached, Probe};
use support::manual_runtime::{ManualRuntime, ms};
use support::recording_terminal::RecordingTerminal;

/// A writer over a recording terminal whose terminal supports `protocol` images.
fn writer(protocol: Option<ImageProtocol>) -> (TUI, RecordingTerminal, ManualRuntime) {
    let images = TerminalImage::new(|_| None, || 1);
    images.set_capabilities(TerminalCapabilities {
        images: protocol,
        true_color: true,
        hyperlinks: true,
    });
    let terminal = RecordingTerminal::new(10, 3);
    let runtime = ManualRuntime::new();
    let tui = TUI::new(terminal.handle(), runtime.handle(), images, None);
    (tui, terminal, runtime)
}

#[test]
fn startup_queries_cells_only_for_images() {
    let cases = [
        (None, vec!["\x1b[?25l", "\x1b[?25h"]),
        (
            Some(ImageProtocol::Kitty),
            vec!["\x1b[?25l", "\x1b[16t", "\x1b[?25h"],
        ),
        (
            Some(ImageProtocol::Iterm2),
            vec!["\x1b[?25l", "\x1b[16t", "\x1b[?25h"],
        ),
    ];
    for (protocol, writes) in cases {
        let (tui, terminal, _runtime) = writer(protocol);
        tui.start().unwrap();
        tui.stop().unwrap();
        assert_eq!(terminal.writes(), writes, "{protocol:?}");
        assert_eq!(terminal.events(), ["start", "stop"], "{protocol:?}");
    }
}

#[test]
fn stop_restores_cursor_after_last_content() {
    let marked_first = format!("a{CURSOR_MARKER}");
    let marked_last = format!("c{CURSOR_MARKER}");
    let cases: [(Vec<&str>, Vec<&str>); 4] = [
        (vec![], vec!["\x1b[?25h"]),
        (vec!["a"], vec!["\x1b[1B", "\r\n", "\x1b[?25h"]),
        (
            vec![&marked_first, "b", "c"],
            vec!["\x1b[3B", "\r\n", "\x1b[?25h"],
        ),
        (
            vec!["a", "b", &marked_last],
            vec!["\x1b[1B", "\r\n", "\x1b[?25h"],
        ),
    ];
    for (lines, writes) in cases {
        let (tui, terminal, runtime) = writer(None);
        let probe = Probe::shared(&lines);
        tui.add_child(probe);
        tui.request_render(false);
        runtime.settle().unwrap();
        terminal.clear_writes();
        tui.stop().unwrap();
        assert_eq!(terminal.writes(), writes, "{lines:?}");
        assert_eq!(terminal.events(), ["stop"], "{lines:?}");
    }
}

#[test]
fn hardware_cursor_defaults_and_override() {
    let cases = [
        (None, None, false),
        (None, Some(false), false),
        (None, Some(true), true),
        (Some("0"), None, false),
        (Some("0"), Some(false), false),
        (Some("0"), Some(true), true),
        (Some("1"), None, true),
        (Some("1"), Some(false), false),
        (Some("1"), Some(true), true),
        (Some("true"), None, false),
        (Some("true"), Some(false), false),
        (Some("true"), Some(true), true),
    ];
    for (environment, explicit, expected) in cases {
        let runtime = ManualRuntime::new();
        if let Some(value) = environment {
            runtime.set_environment("MAESTRO_HARDWARE_CURSOR", value);
        }
        let terminal = RecordingTerminal::new(10, 3);
        let tui = TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            explicit,
        );
        assert_eq!(
            tui.get_show_hardware_cursor(),
            expected,
            "{environment:?} {explicit:?}"
        );
    }
}

#[test]
fn clear_on_shrink_defaults_to_off() {
    for (environment, expected) in [
        (None, false),
        (Some("0"), false),
        (Some("1"), true),
        (Some("true"), false),
    ] {
        let runtime = ManualRuntime::new();
        if let Some(value) = environment {
            runtime.set_environment("MAESTRO_CLEAR_ON_SHRINK", value);
        }
        let terminal = RecordingTerminal::new(10, 3);
        let tui = TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        assert_eq!(tui.get_clear_on_shrink(), expected, "{environment:?}");
    }
}

#[test]
fn cursor_visibility_setter_skips_unchanged_values() {
    let cases: [(&[bool], &[&str], usize, bool); 3] = [
        (&[false], &[], 0, false),
        (&[true, true], &[], 1, true),
        (&[true, false], &["\x1b[?25l"], 1, false),
    ];
    for (changes, writes, requests, shown) in cases {
        let (tui, terminal, runtime) = writer(None);
        for enabled in changes {
            tui.set_show_hardware_cursor(*enabled).unwrap();
        }
        assert_eq!(terminal.writes(), writes, "{changes:?}");
        assert_eq!(runtime.pending(), requests, "{changes:?}");
        assert_eq!(tui.get_show_hardware_cursor(), shown, "{changes:?}");
    }
}

/// What happened, and when in milliseconds.
#[derive(Debug, PartialEq, Eq)]
enum Event {
    /// The component rendered at this time.
    Render(u64),
    /// The test looked at the render count at this time.
    Observed(u64, usize),
}

/// Runs one scheduling variant on a controlled clock and returns what happened.
fn schedule_variant(variant: &str) -> (Vec<Event>, usize) {
    let (tui, _terminal, runtime) = writer(None);
    let events: Rc<RefCell<Vec<Event>>> = Rc::default();
    let probe = Probe::shared(&["one"]);
    tui.add_child(probe.clone());
    let during_render = variant == "during_render";
    let (log, clock, writer) = (Rc::clone(&events), runtime.clone(), tui.clone());
    probe.on_render(move |count| {
        log.borrow_mut().push(Event::Render(clock.millis()));
        if during_render && count == 1 {
            writer.request_render(false);
        }
    });
    runtime.advance_to(ms(100));
    succeeds(tui.start());
    runtime.run_due();
    tui.request_render(false);
    tui.request_render(false);
    runtime.advance_to(ms(115));
    events
        .borrow_mut()
        .push(Event::Observed(115, probe.renders.get()));
    match variant {
        "force" => tui.request_render(true),
        "stop" => succeeds(tui.stop()),
        "restart" => {
            succeeds(tui.stop());
            succeeds(tui.start());
        }
        "force_then_restart" => {
            tui.request_render(true);
            succeeds(tui.stop());
            succeeds(tui.start());
        }
        _ => {}
    }
    runtime.run_due();
    runtime.advance_to(ms(116));
    events
        .borrow_mut()
        .push(Event::Observed(116, probe.renders.get()));
    runtime.advance_to(ms(132));
    probe.clear_callbacks();
    let seen = events.take();
    (seen, runtime.pending())
}

#[test]
fn render_requests_coalesce_cancel_and_survive_reentry() {
    use Event::{Observed, Render};
    let cases = [
        (
            "coalesce",
            vec![Render(100), Observed(115, 1), Render(116), Observed(116, 2)],
        ),
        (
            "force",
            vec![Render(100), Observed(115, 1), Render(115), Observed(116, 2)],
        ),
        (
            "stop",
            vec![Render(100), Observed(115, 1), Observed(116, 1)],
        ),
        (
            "restart",
            vec![Render(100), Observed(115, 1), Render(116), Observed(116, 2)],
        ),
        (
            "force_then_restart",
            vec![Render(100), Observed(115, 1), Render(116), Observed(116, 2)],
        ),
        (
            "during_render",
            vec![Render(100), Observed(115, 1), Render(116), Observed(116, 2)],
        ),
    ];
    for (variant, events) in cases {
        assert_eq!(schedule_variant(variant), (events, 0), "{variant}");
    }
    assert_children_edited_during_a_walk_follow_the_live_list();
    assert_invalidation_while_rendering_reaches_later_siblings_in_the_same_frame();
}

/// Which walk over the children a component edits the list during.
#[derive(Clone, Copy, Debug)]
enum Walk {
    /// A frame renders every child.
    Render,
    /// The writer invalidates every child.
    Invalidate,
}

/// Runs `walk` over three children where the first adds a fourth (`add`) or removes the
/// second (`!add`) while it is visited; returns how often each child was visited.
fn walk_with_edit(walk: Walk, add: bool) -> Vec<usize> {
    let (tui, _terminal, runtime) = writer(None);
    let children: Vec<_> = ["a", "b", "c", "d"]
        .map(|line| Probe::shared(&[line]))
        .into();
    for child in &children[..3] {
        tui.add_child(child.clone());
    }
    let (writer, added, removed): (_, ComponentHandle, ComponentHandle) =
        (tui.clone(), children[3].clone(), children[1].clone());
    let edit = move || {
        if add {
            writer.add_child(added.clone());
        } else {
            writer.remove_child(&removed);
        }
    };
    match walk {
        Walk::Render => children[0].on_render(move |_| edit()),
        Walk::Invalidate => children[0].on_invalidate(edit),
    }
    match walk {
        Walk::Render => {
            tui.request_render(false);
            succeeds(runtime.settle());
        }
        Walk::Invalidate => tui.invalidate(),
    }
    children
        .iter()
        .map(|child| match walk {
            Walk::Render => child.renders.get(),
            Walk::Invalidate => child.invalidated.get(),
        })
        .collect()
}

/// A child added during a walk is visited in it and one removed before its turn is not.
fn assert_children_edited_during_a_walk_follow_the_live_list() {
    for walk in [Walk::Render, Walk::Invalidate] {
        assert_eq!(walk_with_edit(walk, true), [1, 1, 1, 1], "{walk:?} add");
        assert_eq!(walk_with_edit(walk, false), [1, 0, 1, 0], "{walk:?} remove");
    }
}

/// A component that invalidates the writer while it renders clears the cached rendering of
/// a later component of its container before that component renders, so the same frame
/// draws the later component's new text.
fn assert_invalidation_while_rendering_reaches_later_siblings_in_the_same_frame() {
    let (tui, terminal, runtime) = writer(None);
    let trigger = Probe::shared(&["trigger"]);
    let sibling = Cached::shared("old");
    let container = Rc::new(Container::new());
    container.add_child(trigger.clone());
    container.add_child(sibling.clone());
    tui.add_child(container);
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(terminal.writes().concat().matches("old").count(), 1);

    sibling.set_text("new");
    terminal.clear_writes();
    let writer = tui.clone();
    trigger.on_render(move |_| writer.invalidate());
    tui.request_render(false);
    succeeds(runtime.settle());
    let drawn = terminal.writes().concat();
    assert!(
        drawn.contains("new") && !drawn.contains("old"),
        "the frame replaces the cached text: {drawn:?}"
    );
    trigger.clear_callbacks();
}
