//! Actual frame-writer cursor effects and containing-owner teardown.
use super::*;
use crate::recording_terminal;
use maestro_tui::{CURSOR_MARKER, TUI, TerminalImage};
use serde_json::json;

/// A complete writer run.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    input: Input,
    expected: Expected,
}
/// Writer construction and exact operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    focused: bool,
    width: usize,
    height: usize,
    ops: Vec<Operation>,
}
/// Ordered writer observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    frames: Vec<Frame>,
}
/// One forced frame and hardware effects.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    lines: Vec<String>,
    effects: Vec<Effect>,
    hardware: String,
    marker: bool,
}
/// Exact terminal write or hardware cursor visibility.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "lowercase")]
enum Effect {
    Write { data: String },
    Show {},
    Hide {},
}

/// Replays both focus paths using the delivered writer and shared controlled runtime.
pub(super) fn replay() {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(maestro_tui::get_keybindings());
    maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
        maestro_app::presentation_data::keybindings::KEYBINDINGS.clone(),
        vec![],
    ));
    let cases: Vec<Case> =
        super::on_this_platform(include_str!("../fixtures/tree-writer-focus.json"));
    assert_eq!(cases.len(), 2);
    for case in cases {
        replay_case(&case);
    }
}
/// Holds the containing writer until all observations complete.
fn replay_case(case: &Case) {
    assert_eq!(
        case.test,
        "label_focus_reaches_the_input_cursor_through_the_writer"
    );
    let host = manual_runtime::ManualRuntime::new();
    let terminal = recording_terminal::RecordingTerminal::new(case.input.width, case.input.height);
    let tui = TUI::new(
        terminal.handle(),
        host.handle(),
        TerminalImage::new(|_| None, || 1),
        Some(true),
    );
    let (_, theme) = published::published(&palette::palette("dark"));
    let entry = json!({"id":"a","parentId":null,"type":"message","message":{"role":"user","content":"hello"}});
    let node = maestro_app::SessionTreeNode {
        entry: &entry,
        children: vec![],
        label: None,
        label_timestamp: None,
    };
    let capture = Rc::new(());
    let weak_capture = Rc::downgrade(&capture);
    let callback = Rc::new(move || {
        let _ = &capture;
    });
    let panel = Rc::new(
        TreeSelectorComponent::new(
            &[node],
            TreeSelectorCallbacks {
                on_select: Rc::new(|_| {}),
                on_cancel: callback,
                on_label_change: None,
            },
            TreeSelectorOptions {
                current_leaf_id: Some("a".into()),
                terminal_height: case.input.height,
                initial_selected_id: None,
                initial_filter_mode: None,
                theme,
                runtime: host.handle(),
                styles_enabled: true,
                now: Some(Rc::new(|| {
                    std::time::UNIX_EPOCH + std::time::Duration::from_millis(1_791_981_296_789)
                })),
            },
        )
        .unwrap(),
    );
    tui.add_child(panel.clone());
    assert_eq!(case.input.ops.len(), case.expected.frames.len());
    match &case.input.ops[0] {
        Operation::Change(c) => assert_eq!(c.focus, Some(case.input.focused)),
        Operation::Key(_) => panic!("initial focus missing"),
    }
    frames(&panel, &tui, &terminal, &host, case);
    teardown(panel, tui, &host, &weak_capture);
}
/// Proves release through the actual containing owner.
fn teardown(
    panel: Rc<TreeSelectorComponent>,
    tui: TUI,
    host: &manual_runtime::ManualRuntime,
    weak_capture: &std::rc::Weak<()>,
) {
    let weak_panel = Rc::downgrade(&panel);
    let weak_list = Rc::downgrade(panel.get_tree_list());
    drop(panel);
    assert!(weak_panel.upgrade().is_some());
    drop(tui);
    assert!(weak_panel.upgrade().is_none());
    assert!(weak_list.upgrade().is_none());
    assert!(weak_capture.upgrade().is_none());
    assert_eq!(host.pending(), 0);
}
/// Observes each actual forced frame without retaining previous writes.
fn frames(
    panel: &Rc<TreeSelectorComponent>,
    tui: &TUI,
    terminal: &recording_terminal::RecordingTerminal,
    host: &manual_runtime::ManualRuntime,
    case: &Case,
) {
    for (op, frame) in case.input.ops.iter().zip(&case.expected.frames) {
        match op {
            Operation::Key(data) => panel.handle_input(data).unwrap(),
            Operation::Change(c) => {
                let focused = c.focus.unwrap();
                tui.set_focus(focused.then(|| panel.clone() as Rc<dyn Component>));
            }
        }
        terminal.clear_writes();
        tui.request_render(true);
        for result in host.advance_to(std::time::Duration::ZERO) {
            result.unwrap();
        }
        let lines = panel.render(case.input.width);
        assert_eq!(lines, frame.lines);
        assert_eq!(
            lines.iter().any(|s| s.contains(CURSOR_MARKER)),
            frame.marker
        );
        let expected: Vec<_> = frame
            .effects
            .iter()
            .map(|effect| match effect {
                Effect::Write { data } => data.clone(),
                Effect::Show {} => "\x1b[?25h".into(),
                Effect::Hide {} => "\x1b[?25l".into(),
            })
            .collect();
        assert_eq!(terminal.writes(), expected);
        assert!(!terminal.writes().concat().contains(CURSOR_MARKER));
        assert_eq!(frame.hardware, if frame.marker { "show" } else { "hide" });
    }
}
