//! Input reaches the focused component only after listeners and cell-size replies.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_tui::images::terminal_image::CellDimensions;
use maestro_tui::tui::{InputListener, InputListenerResult};
use maestro_tui::{Component, Container, TUI, TerminalImage};
use serde::Deserialize;

use crate::support::checks::parse;
use crate::support::checks::succeeds;
use crate::support::components::{Capabilities, Probe};
use crate::support::manual_runtime::ManualRuntime;
use crate::support::recording_terminal::RecordingTerminal;
use crate::support::scene::Scene;

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
    let _input = crate::input_support::globals();
    let scene = focused_scene(ghostty());
    succeeds(scene.tui.start());
    scene.terminal.send_input("\x1b");
    assert_eq!(*scene.probe.inputs.borrow(), ["\x1b"]);
    scene.stop();
    scene.assert_recorded("bare_escape");
}

#[test]
fn maestro_frames_consumes_cell_size_responses_and_still_forwards_later_user_input() {
    let _input = crate::input_support::globals();
    let images = ghostty();
    let scene = focused_scene(images.clone());
    succeeds(scene.tui.start());
    scene.terminal.send_input("\x1b[6;20;10t");
    assert!(scene.probe.inputs.borrow().is_empty());
    assert_eq!(
        images.get_cell_dimensions(),
        CellDimensions {
            width_px: 10,
            height_px: 20
        }
    );
    scene.terminal.send_input("q");
    assert_eq!(*scene.probe.inputs.borrow(), ["q"]);
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
    probe: Rc<Probe>,
    /// The shared image state.
    images: TerminalImage,
    /// What the scenario undoes when it ends, in the order it was registered.
    releases: RefCell<Vec<Box<dyn FnOnce()>>>,
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
            releases: RefCell::default(),
        }
    }

    /// Registers `listener` and undoes the registration when the scenario is released.
    fn listen(&self, listener: InputListener) {
        let remove = self.tui.add_input_listener(listener);
        self.on_release(remove);
    }

    /// Runs `cleanup` when the scenario is released.
    fn on_release(&self, cleanup: impl FnOnce() + 'static) {
        self.releases.borrow_mut().push(Box::new(cleanup));
    }

    /// Ends the scenario: undoes what it registered, so nothing a callback captured outlives
    /// the writer, and drops the writer.
    fn release(self) {
        for cleanup in self.releases.take() {
            cleanup();
        }
    }
}

#[test]
fn cell_replies_consume_only_recognized_chunks() {
    let _input = crate::input_support::globals();
    for case in routing().cell_replies {
        let rig = Rig::new();
        rig.terminal.send_input(&case.data);
        assert_eq!(*rig.probe.inputs.borrow(), case.inputs, "{:?}", case.data);
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
            rig.probe.invalidated.get(),
            case.invalidated,
            "{:?}",
            case.data
        );
    }
}

#[test]
fn debug_and_release_routing_preserve_order() {
    let _input = crate::input_support::globals();
    for case in routing().debug_release {
        let rig = Rig::new();
        rig.probe.wants_release.set(case.wants_release);
        let calls = Rc::new(RefCell::new(0));
        if case.debug {
            let counter = Rc::clone(&calls);
            rig.tui
                .set_on_debug(Some(Rc::new(move || *counter.borrow_mut() += 1)));
        }
        rig.terminal.send_input(&case.data);
        assert_eq!(*rig.probe.inputs.borrow(), case.inputs, "{:?}", case.data);
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
    let _input = crate::input_support::globals();
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
        rig.on_release(move || {
            if let Some(dispose) = own_disposer.take() {
                dispose();
            }
        });
        if case.variant != "live_add" {
            rig.listen(Rc::clone(&second));
        }
        if case.variant == "identity_add" {
            rig.listen(Rc::clone(&second));
        }
        rig.terminal.send_input("original");
        let log = seen.borrow();
        let seen_now: Vec<(&str, &str)> = log
            .iter()
            .map(|(name, data)| (*name, data.as_str()))
            .collect();
        assert_eq!(seen_now, case.seen, "{}", case.variant);
        assert_eq!(*rig.probe.inputs.borrow(), case.inputs, "{}", case.variant);
        drop(log);
        rig.release();
        drop(second);
        assert_released(&seen, case.variant);
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
    assert_eq!(*rig.probe.inputs.borrow(), ["first", "second"]);
}

#[test]
fn focus_transitions_update_capability_flags() {
    let _input = crate::input_support::globals();
    let rig = Rig::new();
    let other = Probe::shared(&[]);
    other.capabilities.set(Capabilities::FocusOnly);
    rig.tui.set_focus(Some(rig.probe.clone()));
    assert!(rig.probe.focus.get());
    rig.tui.set_focus(Some(other.clone()));
    assert_eq!((rig.probe.focus.get(), other.focus.get()), (false, true));
    rig.tui.set_focus(None);
    assert!(!other.focus.get());
    rig.terminal.send_input("x");
    assert!(rig.probe.inputs.borrow().is_empty());
    assert_focus_positions_cursor();
    assert_focus_change_during_render_applies_to_that_frame();
    assert_nested_component_focusing_itself_is_flagged_at_once();
    assert_component_changes_survive_its_own_callback();
    assert_container_invalidation_reaches_the_running_child();
    assert_invalidation_leaves_a_focused_component_outside_the_walk_alone();
}

/// A component that is handling input or rendering moves focus and invalidates through the
/// writer: the other components change inside its callback, and so does the running one.
fn assert_component_changes_survive_its_own_callback() {
    let rig = Rig::new();
    let (next, sibling) = (Probe::shared(&[]), Probe::shared(&[]));
    rig.tui.add_child(sibling.clone());
    let inside = Rc::new(Cell::new((false, 0, 0)));
    let (tui, target, other, own, seen) = (
        rig.tui.clone(),
        next.clone(),
        sibling.clone(),
        Rc::downgrade(&rig.probe),
        Rc::clone(&inside),
    );
    rig.probe.on_input(move |_| {
        tui.set_focus(Some(target.clone()));
        tui.invalidate();
        let running = own.upgrade().map_or(0, |own| own.invalidated.get());
        seen.set((target.focus.get(), other.invalidated.get(), running));
    });
    rig.terminal.send_input("x");
    assert_eq!(
        inside.get(),
        (true, 1, 1),
        "the new focus and the invalidation of the sibling and of the running component happen \
         inside the callback"
    );
    assert!(!rig.probe.focus.get());
    assert!(next.focus.get());
    assert_eq!(rig.probe.invalidated.get(), 1);

    let writer = rig.tui.clone();
    rig.probe.on_render(move |_| writer.invalidate());
    rig.tui.request_render(false);
    succeeds(rig.runtime.settle());
    assert_eq!(rig.probe.invalidated.get(), 2);
    assert_eq!(sibling.invalidated.get(), 2);
}

/// A child that handles input inside a container is invalidated at once by each call to the
/// container's `invalidate`, so two calls invalidate it twice.
fn assert_container_invalidation_reaches_the_running_child() {
    let rig = Rig::new();
    rig.tui.clear();
    let child = Probe::shared(&[]);
    let container = Rc::new(Container::new());
    container.add_child(child.clone());
    rig.tui.add_child(container.clone());
    rig.tui.set_focus(Some(child.clone()));
    let inside = Rc::new(Cell::new((0, 0)));
    let (walk, own, seen) = (container, Rc::downgrade(&child), Rc::clone(&inside));
    child.on_input(move |_| {
        let count = || own.upgrade().map_or(0, |own| own.invalidated.get());
        walk.invalidate();
        let once = count();
        walk.invalidate();
        seen.set((once, count()));
    });
    rig.terminal.send_input("x");
    assert_eq!(
        inside.get(),
        (1, 2),
        "each call invalidates the running child before it returns"
    );
    assert_eq!(child.invalidated.get(), 2);
}

/// The writer invalidates the components it contains: a focused component that is not one
/// of them is left alone when it asks the writer to invalidate.
fn assert_invalidation_leaves_a_focused_component_outside_the_walk_alone() {
    let rig = Rig::new();
    let outside = Probe::shared(&[]);
    rig.tui.set_focus(Some(outside.clone()));
    let writer = rig.tui.clone();
    outside.on_input(move |_| writer.invalidate());
    rig.terminal.send_input("x");
    assert_eq!(
        (outside.invalidated.get(), rig.probe.invalidated.get()),
        (0, 1)
    );
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
    editor.emits_marker.set(true);
    tui.add_child(editor.clone());
    tui.set_focus(Some(editor.clone()));
    let writer = tui.clone();
    editor.on_render(move |_| writer.set_focus(None));
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes(),
        ["\x1b[?2026habc\x1b[0m\x1b]8;;\x07\x1b[?2026l", "\x1b[?25l"],
        "clearing focus while rendering drops the marker from that frame"
    );
    assert!(!editor.focus.get());

    terminal.clear_writes();
    tui.clear();
    let taker = Probe::shared(&["abc"]);
    taker.emits_marker.set(true);
    tui.add_child(taker.clone());
    let focus = focus_itself(&tui, &taker, &Rc::default());
    taker.on_render(move |_| focus());
    tui.request_render(false);
    succeeds(runtime.settle());
    assert_eq!(
        terminal.writes().last().map(String::as_str),
        Some("\x1b[?25h"),
        "taking focus while rendering marks the cursor in that frame"
    );
    assert!(taker.focus.get());
}

/// A callback that gives `component` focus and records whether its flag was set by the
/// time the writer returned.
fn focus_itself(tui: &TUI, component: &Rc<Probe>, flagged: &Rc<Cell<bool>>) -> impl Fn() + 'static {
    let (tui, own, flagged) = (tui.clone(), Rc::downgrade(component), Rc::clone(flagged));
    move || {
        if let Some(own) = own.upgrade() {
            tui.set_focus(Some(own.clone()));
            flagged.set(own.focus.get());
        }
    }
}

/// A component inside a container was never added to the writer. When it gives itself
/// focus while it renders and again while it handles input, its flag is set before the
/// writer returns, and it receives the input.
fn assert_nested_component_focusing_itself_is_flagged_at_once() {
    let rig = Rig::new();
    rig.tui.clear();
    let hidden = Probe::shared(&["abc"]);
    let container = Rc::new(Container::new());
    container.add_child(hidden.clone());
    rig.tui.add_child(container);
    let flagged = Rc::new(Cell::new(false));
    let focus = focus_itself(&rig.tui, &hidden, &flagged);
    hidden.on_render(move |_| focus());
    rig.tui.request_render(false);
    succeeds(rig.runtime.settle());
    assert!(flagged.get(), "its flag was set inside its render");
    assert!(hidden.focus.get() && !rig.probe.focus.get());

    hidden.focus.set(false);
    flagged.set(false);
    let focus = focus_itself(&rig.tui, &hidden, &flagged);
    hidden.on_input(move |_| focus());
    rig.terminal.send_input("x");
    assert!(flagged.get(), "its flag was set inside its input handler");
    assert_eq!(*hidden.inputs.borrow(), ["x"]);
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
    editor.emits_marker.set(true);
    let passive = Probe::shared(&["def"]);
    passive.capabilities.set(Capabilities::Passive);
    passive.emits_marker.set(true);
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
    let _input = crate::input_support::globals();
    let rig = Rig::new();
    succeeds(rig.runtime.settle());
    rig.tui.set_focus(None);
    rig.terminal.send_input("x");
    assert!(rig.probe.inputs.borrow().is_empty());
    assert_eq!(rig.runtime.pending(), 0, "ignored input requests no frame");

    let passive = Probe::shared(&[]);
    passive.capabilities.set(Capabilities::Passive);
    rig.tui.set_focus(Some(passive.clone()));
    rig.terminal.send_input("x");
    assert!(passive.inputs.borrow().is_empty());
    assert_eq!(
        rig.runtime.pending(),
        0,
        "a component without input requests no frame"
    );
}

/// One recorded corpus row: the callbacks it registers and the delivery it observes.
#[derive(Deserialize)]
struct CorpusCase {
    /// The scenario name the registration table dispatches on.
    name: String,
    /// Chunks the terminal delivers in order.
    inputs: Vec<String>,
    /// What every visited callback received, in order.
    seen: Vec<(String, String)>,
    /// What the focused component received, in order.
    delivered: Vec<String>,
}

/// The recorded listener corpus.
fn listener_corpus() -> Vec<CorpusCase> {
    parse::<CorpusRows>(include_str!("fixtures/listener_order.json")).rows
}

/// The recorded listener corpus file.
#[derive(Default, Deserialize)]
struct CorpusRows {
    /// One entry per scenario.
    rows: Vec<CorpusCase>,
}

/// A listener for `label` that records each text and answers with `answer`.
fn listener(
    label: &'static str,
    seen: &Seen,
    answer: impl Fn(&str) -> InputListenerResult + 'static,
) -> InputListener {
    let seen = Rc::clone(seen);
    Rc::new(move |data: &str| {
        seen.borrow_mut().push((label, data.to_owned()));
        answer(data)
    })
}

/// Register one recording listener per label.
fn passing(rig: &Rig, seen: &Seen, labels: &[&'static str]) {
    for label in labels {
        rig.listen(listener(label, seen, |_| InputListenerResult::Pass));
    }
}

/// Register one callback twice, then remove it twice through the handle at `which`.
fn duplicate(rig: &Rig, seen: &Seen, which: usize) {
    let shared = listener("only", seen, |_| InputListenerResult::Pass);
    let removers = [
        rig.tui.add_input_listener(Rc::clone(&shared)),
        rig.tui.add_input_listener(shared),
    ];
    removers[which]();
    removers[which]();
    rig.on_release(move || removers[1 - which]());
}

/// Register the identity and action rows of the corpus.
fn register_identity(name: &str, rig: &Rig, seen: &Seen) -> bool {
    let tui = &rig.tui;
    match name {
        "no_listeners" => {}
        "registration_order" => passing(rig, seen, &["z", "a"]),
        "distinct_identity" => passing(rig, seen, &["same", "same"]),
        "duplicate_unsubscribe_0" => duplicate(rig, seen, 0),
        "duplicate_unsubscribe_1" => duplicate(rig, seen, 1),
        "duplicate_keeps_position" => {
            let z = listener("z", seen, |_| InputListenerResult::Pass);
            rig.listen(Rc::clone(&z));
            passing(rig, seen, &["a"]);
            rig.listen(z);
        }
        "direct_remove_absent" => {
            let absent = listener("absent", seen, |_| InputListenerResult::Pass);
            tui.remove_input_listener(&absent);
            passing(rig, seen, &["kept"]);
        }
        "old_handle_removes_readded" => {
            let same = listener("same", seen, |_| InputListenerResult::Pass);
            let old = tui.add_input_listener(Rc::clone(&same));
            tui.remove_input_listener(&same);
            rig.listen(Rc::clone(&same));
            old();
        }
        "dropped_handle_keeps_registration" => passing(rig, seen, &["kept"]),
        _ => return false,
    }
    true
}

/// Register the rows where a callback changes the registrations while it runs.
///
/// The callbacks hold the writer, so the rig releases them with the scenario.
fn register_live(name: &str, rig: &Rig, seen: &Seen) -> bool {
    match name {
        "live_add" => {
            let b = listener("b", seen, |_| InputListenerResult::Pass);
            let writer = rig.tui.clone();
            rig.listen(listener("a", seen, move |_| {
                drop(writer.add_input_listener(Rc::clone(&b)));
                InputListenerResult::Pass
            }));
        }
        "live_remove" => {
            let b = listener("b", seen, |_| InputListenerResult::Pass);
            let writer = rig.tui.clone();
            let removed = Rc::clone(&b);
            rig.listen(listener("a", seen, move |_| {
                writer.remove_input_listener(&removed);
                InputListenerResult::Pass
            }));
            rig.listen(b);
        }
        "remove_self" => {
            passing(rig, seen, &["z"]);
            let disposer: Disposer = Rc::default();
            let holder = Rc::clone(&disposer);
            let acting = listener("a", seen, move |_| {
                if let Some(dispose) = holder.borrow().as_ref() {
                    dispose();
                }
                InputListenerResult::Pass
            });
            *disposer.borrow_mut() = Some(rig.tui.add_input_listener(acting));
            rig.on_release(move || {
                if let Some(dispose) = disposer.take() {
                    dispose();
                }
            });
            passing(rig, seen, &["b"]);
        }
        _ => return false,
    }
    true
}

/// Register the rows where a callback re-registers an identity.
fn register_readd(name: &str, rig: &Rig, seen: &Seen) -> bool {
    match name {
        "readd_moves_to_tail" => {
            let b = listener("b", seen, |_| InputListenerResult::Pass);
            let writer = rig.tui.clone();
            let moved = Rc::clone(&b);
            rig.listen(listener("a", seen, move |_| {
                writer.remove_input_listener(&moved);
                drop(writer.add_input_listener(Rc::clone(&moved)));
                InputListenerResult::Pass
            }));
            rig.listen(b);
            passing(rig, seen, &["c"]);
        }
        "self_readd_once" => {
            let holder: Rc<RefCell<Option<InputListener>>> = Rc::default();
            let own = Rc::clone(&holder);
            let writer = rig.tui.clone();
            let repeated = Rc::new(Cell::new(false));
            let acting = listener("a", seen, move |_| {
                if repeated.replace(true) {
                    return InputListenerResult::Pass;
                }
                if let Some(this) = own.borrow_mut().take() {
                    writer.remove_input_listener(&this);
                    drop(writer.add_input_listener(Rc::clone(&this)));
                }
                InputListenerResult::Pass
            });
            *holder.borrow_mut() = Some(Rc::clone(&acting));
            rig.listen(acting);
            rig.on_release(move || drop(holder.take()));
            passing(rig, seen, &["b"]);
        }
        _ => return false,
    }
    true
}

/// Register the rows where a callback answers with a result.
fn register_answers(name: &str, rig: &Rig, seen: &Seen) -> bool {
    match name {
        "consume_short_circuits" => {
            rig.listen(listener("a", seen, |_| InputListenerResult::Consume));
            passing(rig, seen, &["b"]);
        }
        "empty_final_stops" => {
            rig.listen(listener("a", seen, |_| {
                InputListenerResult::Replace(String::new())
            }));
            passing(rig, seen, &["b"]);
        }
        "empty_then_rewrite" => {
            rig.listen(listener("a", seen, |_| {
                InputListenerResult::Replace(String::new())
            }));
            rig.listen(listener("b", seen, |_| {
                InputListenerResult::Replace("recovered".to_owned())
            }));
        }
        "replace_preserves_bytes" => {
            rig.listen(listener("a", seen, |_| {
                InputListenerResult::Replace("é\t\x1b[31mλ".to_owned())
            }));
            passing(rig, seen, &["b"]);
        }
        _ => return false,
    }
    true
}

#[test]
fn listener_corpus_preserves_focus_delivery() {
    let _input = crate::input_support::globals();
    for case in listener_corpus() {
        let rig = Rig::new();
        let seen: Seen = Rc::default();
        let registered = register_identity(&case.name, &rig, &seen)
            || register_live(&case.name, &rig, &seen)
            || register_readd(&case.name, &rig, &seen)
            || register_answers(&case.name, &rig, &seen);
        assert!(registered, "{}", case.name);
        for data in &case.inputs {
            rig.terminal.send_input(data);
        }
        let visited: Vec<(String, String)> = seen
            .borrow()
            .iter()
            .map(|(label, data)| ((*label).to_owned(), data.clone()))
            .collect();
        assert_eq!(visited, case.seen, "{}", case.name);
        assert_eq!(*rig.probe.inputs.borrow(), case.delivered, "{}", case.name);
        rig.release();
        assert_released(&seen, &case.name);
    }
}

/// Checks that no callback of the scenario `name` outlived its writer: every clone of the
/// shared record the listeners captured is gone.
fn assert_released(seen: &Seen, name: &str) {
    assert_eq!(
        Rc::strong_count(seen),
        1,
        "{name}: a listener outlives the writer"
    );
}

#[test]
fn duplicate_input_unsubscribers_remove_the_current_registration() {
    let _input = crate::input_support::globals();
    for which in [0, 1] {
        let rig = Rig::new();
        let seen: Seen = Rc::default();
        let shared = listener("only", &seen, |_| InputListenerResult::Pass);
        let removers = [
            rig.tui.add_input_listener(Rc::clone(&shared)),
            rig.tui.add_input_listener(shared),
        ];
        rig.terminal.send_input("first");
        removers[which]();
        removers[which]();
        rig.terminal.send_input("second");
        assert_eq!(*seen.borrow(), [("only", "first".to_owned())], "{which}");
        assert_eq!(*rig.probe.inputs.borrow(), ["first", "second"], "{which}");
        rig.on_release(move || removers[1 - which]());
        rig.release();
        assert_released(&seen, &which.to_string());
    }
}
