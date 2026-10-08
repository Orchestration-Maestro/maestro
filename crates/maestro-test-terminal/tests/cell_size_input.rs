//! Input reaches the focused component only after listeners and cell-size replies.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::images::terminal_image::CellDimensions;
use maestro_tui::tui::{InputListener, InputListenerResult};
use maestro_tui::{Container, TUI, TerminalImage};
use serde::Deserialize;

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod components;
    pub mod manual_runtime;
    pub mod recording_terminal;
    pub mod scene;
    pub mod virtual_terminal;
}
use support::checks::parse;
use support::checks::succeeds;
use support::components::{Capabilities, Probe};
use support::manual_runtime::ManualRuntime;
use support::recording_terminal::RecordingTerminal;
use support::scene::Scene;

/// The pixel size of a cell before any reply.
const DEFAULT_CELLS: CellDimensions = CellDimensions {
    width_px: 9,
    height_px: 18,
};

/// An image terminal whose cell size the writer asks for at startup.
fn ghostty() -> TerminalImage {
    TerminalImage::new(
        |key| (key == "TERM_PROGRAM").then(|| "ghostty".to_owned()),
        || 1,
    )
}

/// A scene whose only component has focus and the default cell size.
fn focused_scene(images: TerminalImage) -> Scene {
    images.set_cell_dimensions(DEFAULT_CELLS);
    let scene = Scene::with_images(80, 24, images);
    scene.tui.set_focus(Some(scene.probe.clone()));
    scene
}

#[test]
fn maestro_frames_forwards_bare_escape_even_when_a_cell_size_query_was_sent_at_startup() {
    let scene = focused_scene(ghostty());
    succeeds(scene.tui.start());
    scene.terminal.send_input("\x1b");
    assert_eq!(scene.probe.borrow().inputs, ["\x1b"]);
    scene.stop();
    scene.assert_recorded("bare_escape");
}

#[test]
fn maestro_frames_consumes_cell_size_responses_and_still_forwards_later_user_input() {
    let images = ghostty();
    let scene = focused_scene(images.clone());
    succeeds(scene.tui.start());
    scene.terminal.send_input("\x1b[6;20;10t");
    assert!(scene.probe.borrow().inputs.is_empty());
    assert_eq!(
        images.get_cell_dimensions(),
        CellDimensions {
            width_px: 10,
            height_px: 20
        }
    );
    scene.terminal.send_input("q");
    assert_eq!(scene.probe.borrow().inputs, ["q"]);
    scene.stop();
    scene.assert_recorded("cell_reply");
}

/// Inputs the writer must route and what they must do.
#[derive(Default, Deserialize)]
struct Routing {
    /// Chunks that may be cell-size replies.
    cell_replies: Vec<CellReplyCase>,
    /// Chunks that may be release events or the debug key.
    debug_release: Vec<DebugReleaseCase>,
}

/// One chunk offered to a focused component and the cell size it leaves behind.
#[derive(Deserialize)]
struct CellReplyCase {
    /// The chunk the terminal delivers.
    data: String,
    /// What the component receives.
    inputs: Vec<String>,
    /// Cell width afterwards.
    width_px: u32,
    /// Cell height afterwards.
    height_px: u32,
    /// How many times the component was invalidated.
    invalidated: usize,
}

/// One chunk offered with a debug callback and a release preference.
#[derive(Deserialize)]
struct DebugReleaseCase {
    /// The chunk the terminal delivers.
    data: String,
    /// Whether the component asks for key-release events.
    wants_release: bool,
    /// Whether a debug callback is set.
    debug: bool,
    /// What the component receives.
    inputs: Vec<String>,
    /// How many times the debug callback ran.
    debug_calls: usize,
}

/// The recorded routing cases.
fn routing() -> Routing {
    parse(include_str!("fixtures/input_routing.json"))
}

/// A started writer over a recording terminal whose only component has focus.
struct Rig {
    /// The writer under test.
    tui: TUI,
    /// The recording terminal.
    terminal: RecordingTerminal,
    /// The controlled host.
    runtime: ManualRuntime,
    /// The focused component.
    probe: Rc<RefCell<Probe>>,
    /// The shared image state.
    images: TerminalImage,
}

impl Rig {
    /// Starts a writer whose only component has focus.
    fn new() -> Self {
        let images = TerminalImage::new(|_| None, || 1);
        let terminal = RecordingTerminal::new(10, 3);
        let runtime = ManualRuntime::new();
        let tui = TUI::new(terminal.handle(), runtime.handle(), images.clone(), None);
        let probe = Probe::shared(&[""]);
        tui.add_child(probe.clone());
        tui.set_focus(Some(probe.clone()));
        succeeds(tui.start());
        Self {
            tui,
            terminal,
            runtime,
            probe,
            images,
        }
    }
}

#[test]
fn cell_replies_consume_only_recognized_chunks() {
    for case in routing().cell_replies {
        let rig = Rig::new();
        rig.terminal.send_input(&case.data);
        assert_eq!(rig.probe.borrow().inputs, case.inputs, "{:?}", case.data);
        assert_eq!(
            rig.images.get_cell_dimensions(),
            CellDimensions {
                width_px: case.width_px,
                height_px: case.height_px
            },
            "{:?}",
            case.data
        );
        assert_eq!(
            rig.probe.borrow().invalidated,
            case.invalidated,
            "{:?}",
            case.data
        );
    }
}

#[test]
fn debug_and_release_routing_preserve_order() {
    for case in routing().debug_release {
        let rig = Rig::new();
        rig.probe.borrow_mut().wants_release = case.wants_release;
        let calls = Rc::new(RefCell::new(0));
        if case.debug {
            let counter = Rc::clone(&calls);
            rig.tui
                .set_on_debug(Some(Rc::new(move || *counter.borrow_mut() += 1)));
        }
        rig.terminal.send_input(&case.data);
        assert_eq!(rig.probe.borrow().inputs, case.inputs, "{:?}", case.data);
        assert_eq!(*calls.borrow(), case.debug_calls, "{:?}", case.data);
    }
}

/// Order in which listeners saw input, with the text each saw.
type Seen = Rc<RefCell<Vec<(&'static str, String)>>>;

/// Removes the first listener from the list when called.
type Disposer = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// What a listener list scenario must show.
struct ListenerCase {
    /// How the first listener treats the input and the list.
    variant: &'static str,
    /// What each listener saw, in order.
    seen: Vec<(&'static str, &'static str)>,
    /// What the component received.
    inputs: Vec<&'static str>,
}

/// The first listener of a scenario: it records the input, then acts as `variant` says.
fn first_listener(
    variant: &'static str,
    seen: &Seen,
    tui: &TUI,
    second: &InputListener,
    own_disposer: &Disposer,
) -> InputListener {
    let (seen, tui, second, own_disposer) = (
        Rc::clone(seen),
        tui.clone(),
        Rc::clone(second),
        Rc::clone(own_disposer),
    );
    Rc::new(move |data: &str| {
        seen.borrow_mut().push(("a", data.to_owned()));
        match variant {
            "live_add" => {
                drop(tui.add_input_listener(Rc::clone(&second)));
                InputListenerResult::Pass
            }
            "live_remove" => {
                tui.remove_input_listener(&second);
                InputListenerResult::Pass
            }
            "remove_self" => {
                if let Some(dispose) = own_disposer.borrow().as_ref() {
                    dispose();
                }
                InputListenerResult::Pass
            }
            "consume" => InputListenerResult::Consume,
            "empty_rewrite" => InputListenerResult::Replace(String::new()),
            "rewrite_cell" => InputListenerResult::Replace("\x1b[6;20;10t".to_owned()),
            "rewrite_user" => InputListenerResult::Replace("replacement".to_owned()),
            _ => InputListenerResult::Pass,
        }
    })
}

/// The recorded listener scenarios.
fn listener_cases() -> Vec<ListenerCase> {
    let both = |second: &'static str| vec![("a", "original"), ("b", second)];
    let case = |variant, seen, inputs| ListenerCase {
        variant,
        seen,
        inputs,
    };
    vec![
        case("live_add", both("original"), vec!["original"]),
        case("live_remove", vec![("a", "original")], vec!["original"]),
        case("remove_self", both("original"), vec!["original"]),
        case("consume", vec![("a", "original")], vec![]),
        case("empty_rewrite", both(""), vec![]),
        case("rewrite_cell", both("\x1b[6;20;10t"), vec![]),
        case("rewrite_user", both("replacement"), vec!["replacement"]),
        case("identity_add", both("original"), vec!["original"]),
    ]
}

#[test]
fn listeners_mutate_live_order_before_routing() {
    for case in listener_cases() {
        let rig = Rig::new();
        let seen: Seen = Rc::default();
        let second: InputListener = {
            let seen = Rc::clone(&seen);
            Rc::new(move |data: &str| {
                seen.borrow_mut().push(("b", data.to_owned()));
                InputListenerResult::Pass
            })
        };
        let own_disposer = Disposer::default();
        let first = first_listener(case.variant, &seen, &rig.tui, &second, &own_disposer);
        *own_disposer.borrow_mut() = Some(rig.tui.add_input_listener(first));
        if case.variant != "live_add" {
            drop(rig.tui.add_input_listener(Rc::clone(&second)));
        }
        if case.variant == "identity_add" {
            drop(rig.tui.add_input_listener(Rc::clone(&second)));
        }
        rig.terminal.send_input("original");
        let log = seen.borrow();
        let seen_now: Vec<(&str, &str)> = log
            .iter()
            .map(|(name, data)| (*name, data.as_str()))
            .collect();
        assert_eq!(seen_now, case.seen, "{}", case.variant);
        assert_eq!(rig.probe.borrow().inputs, case.inputs, "{}", case.variant);
    }
    assert_listener_disposal_between_inputs();
}

/// A listener removed between two inputs sees only the first, and removing it again
/// changes nothing.
fn assert_listener_disposal_between_inputs() {
    let rig = Rig::new();
    let seen: Seen = Rc::default();
    let log = Rc::clone(&seen);
    let listener: InputListener = Rc::new(move |data: &str| {
        log.borrow_mut().push(("only", data.to_owned()));
        InputListenerResult::Pass
    });
    let dispose = rig.tui.add_input_listener(Rc::clone(&listener));
    rig.terminal.send_input("first");
    dispose();
    dispose();
    rig.tui.remove_input_listener(&listener);
    rig.terminal.send_input("second");
    assert_eq!(*seen.borrow(), [("only", "first".to_owned())]);
    assert_eq!(rig.probe.borrow().inputs, ["first", "second"]);
}

#[test]
fn focus_transitions_update_capability_flags() {
    let rig = Rig::new();
    let other = Probe::shared(&[]);
    other.borrow_mut().capabilities = Capabilities::FocusOnly;
    rig.tui.set_focus(Some(rig.probe.clone()));
    assert!(rig.probe.borrow().focus.get());
    rig.tui.set_focus(Some(other.clone()));
    assert_eq!(
        (rig.probe.borrow().focus.get(), other.borrow().focus.get()),
        (false, true)
    );
    rig.tui.set_focus(None);
    assert!(!other.borrow().focus.get());
    rig.terminal.send_input("x");
    assert!(rig.probe.borrow().inputs.is_empty());
    assert_focus_positions_cursor();
    assert_focus_change_during_render_applies_to_that_frame();
    assert_component_changes_survive_its_own_callback();
}

/// A component that is handling input or rendering moves focus and invalidates through the
/// writer: other components change inside its callback, and the running component is
/// invalidated as its callback returns.
fn assert_component_changes_survive_its_own_callback() {
    let rig = Rig::new();
    let (next, sibling) = (Probe::shared(&[]), Probe::shared(&[]));
    rig.tui.add_child(sibling.clone());
    let inside = Rc::new(Cell::new((false, 0)));
    let (tui, target, other, seen) = (
        rig.tui.clone(),
        next.clone(),
        sibling.clone(),
        Rc::clone(&inside),
    );
    rig.probe.borrow_mut().on_input = Some(Box::new(move |_| {
        tui.set_focus(Some(target.clone()));
        tui.invalidate();
        seen.set((target.borrow().focus.get(), other.borrow().invalidated));
    }));
    rig.terminal.send_input("x");
    assert_eq!(
        inside.get(),
        (true, 1),
        "the new focus and a sibling's invalidation happen inside the callback"
    );
    assert!(!rig.probe.borrow().focus.get());
    assert!(next.borrow().focus.get());
    assert_eq!(rig.probe.borrow().invalidated, 1);

    let writer = rig.tui.clone();
    rig.probe.borrow_mut().on_render = Some(Box::new(move |_| writer.invalidate()));
    rig.tui.request_render(false);
    succeeds(rig.runtime.settle());
    assert_eq!(rig.probe.borrow().invalidated, 2);
    assert_eq!(sibling.borrow().invalidated, 2);
    assert_nested_focused_component_invalidates_its_container();
}

/// A focused component inside a container that invalidates through the writer neither
/// panics nor is left out: its sibling is invalidated at once and it as its callback
/// returns.
fn assert_nested_focused_component_invalidates_its_container() {
    let rig = Rig::new();
    rig.tui.clear();
    let sibling = Probe::shared(&[]);
    let container = Rc::new(RefCell::new(Container::new()));
    container.borrow_mut().add_child(sibling.clone());
    container.borrow_mut().add_child(rig.probe.clone());
    rig.tui.add_child(container);
    rig.tui.set_focus(Some(rig.probe.clone()));
    let inside = Rc::new(Cell::new(0));
    let (tui, seen) = (rig.tui.clone(), Rc::clone(&inside));
    rig.probe.borrow_mut().on_input = Some(Box::new(move |_| {
        tui.invalidate();
        seen.set(sibling.borrow().invalidated);
    }));
    rig.terminal.send_input("x");
    assert_eq!(
        inside.get(),
        1,
        "the sibling is invalidated inside the callback"
    );
    assert_eq!(rig.probe.borrow().invalidated, 1);
}

/// A focused component that clears focus while it renders no longer marks its cursor in
/// that frame, and a registered component that takes focus while it renders marks it in
/// that frame.
fn assert_focus_change_during_render_applies_to_that_frame() {
    let terminal = RecordingTerminal::new(10, 3);
    let runtime = ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        Some(true),
    );
    let editor = Probe::shared(&["abc"]);
    editor.borrow_mut().emits_marker = true;
    tui.add_child(editor.clone());
    tui.set_focus(Some(editor.clone()));
    let writer = tui.clone();
    editor.borrow_mut().on_render = Some(Box::new(move |_| writer.set_focus(None)));
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes(),
        ["\x1b[?2026habc\x1b[0m\x1b]8;;\x07\x1b[?2026l", "\x1b[?25l"],
        "clearing focus while rendering drops the marker from that frame"
    );
    assert!(!editor.borrow().focus.get());

    terminal.clear_writes();
    tui.clear();
    let taker = Probe::shared(&["abc"]);
    taker.borrow_mut().emits_marker = true;
    tui.add_child(taker.clone());
    let (writer, own) = (tui.clone(), Rc::downgrade(&taker));
    taker.borrow_mut().on_render = Some(Box::new(move |_| {
        if let Some(own) = own.upgrade() {
            writer.set_focus(Some(own));
        }
    }));
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes().last().map(String::as_str),
        Some("\x1b[?25h"),
        "taking focus while rendering marks the cursor in that frame"
    );
    assert!(taker.borrow().focus.get());
    assert_running_component_without_a_known_flag_still_receives_input();
}

/// A component inside a container was never added to the writer, so while it renders its
/// flag cannot be read: focusing itself leaves it unflagged but sends it the input.
fn assert_running_component_without_a_known_flag_still_receives_input() {
    let rig = Rig::new();
    rig.tui.clear();
    let hidden = Probe::shared(&["abc"]);
    let container = Rc::new(RefCell::new(Container::new()));
    container.borrow_mut().add_child(hidden.clone());
    rig.tui.add_child(container);
    let (writer, own) = (rig.tui.clone(), Rc::downgrade(&hidden));
    hidden.borrow_mut().on_render = Some(Box::new(move |_| {
        if let Some(own) = own.upgrade() {
            writer.set_focus(Some(own));
        }
    }));
    rig.tui.request_render(false);
    succeeds(rig.runtime.settle());
    assert!(!hidden.borrow().focus.get(), "its flag was unreachable");
    assert!(!rig.probe.borrow().focus.get(), "the previous focus ended");
    rig.terminal.send_input("x");
    assert_eq!(hidden.borrow().inputs, ["x"]);
}

/// Focusing a component that marks its cursor while focused moves the hardware cursor
/// without redrawing any line; a component that cannot hold focus is never flagged, so its
/// marker never appears and the cursor stays hidden.
fn assert_focus_positions_cursor() {
    let terminal = RecordingTerminal::new(10, 3);
    let runtime = ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        Some(true),
    );
    let editor = Probe::shared(&["abc"]);
    editor.borrow_mut().emits_marker = true;
    let passive = Probe::shared(&["def"]);
    passive.borrow_mut().capabilities = Capabilities::Passive;
    passive.borrow_mut().emits_marker = true;
    tui.add_child(editor.clone());
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes(),
        ["\x1b[?2026habc\x1b[0m\x1b]8;;\x07\x1b[?2026l", "\x1b[?25l"],
        "an unfocused component leaves the cursor hidden"
    );
    terminal.clear_writes();
    tui.set_focus(Some(editor));
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes(),
        ["\x1b[4G", "\x1b[?25h"],
        "focus moves the cursor and draws no line"
    );
    terminal.clear_writes();
    tui.add_child(passive.clone());
    tui.set_focus(Some(passive));
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes().last().map(String::as_str),
        Some("\x1b[?25l"),
        "a component that cannot hold focus is never flagged, so its marker stays out"
    );
}

#[test]
fn missing_input_capability_ignores_input() {
    let rig = Rig::new();
    succeeds(rig.runtime.settle());
    rig.tui.set_focus(None);
    rig.terminal.send_input("x");
    assert!(rig.probe.borrow().inputs.is_empty());
    assert_eq!(rig.runtime.pending(), 0, "ignored input requests no frame");

    let passive = Probe::shared(&[]);
    passive.borrow_mut().capabilities = Capabilities::Passive;
    rig.tui.set_focus(Some(passive.clone()));
    rig.terminal.send_input("x");
    assert!(passive.borrow().inputs.is_empty());
    assert_eq!(
        rig.runtime.pending(),
        0,
        "a component without input requests no frame"
    );
}
