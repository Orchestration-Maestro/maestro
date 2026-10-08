//! Retained frames written through the public terminal and runtime interfaces.

use maestro_tui::tui::ComponentHandle;
use maestro_tui::{Component, KittyOptions, TUI, TerminalImage, delete_kitty_image, encode_kitty};
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
use support::checks::{parse, succeeds};
use support::components::Probe;
use support::manual_runtime::ManualRuntime;
use support::recording_terminal::RecordingTerminal;
use support::scene::Scene;

/// Terminal width of the frame scenarios.
const COLUMNS: usize = 10;
/// Terminal height of the frame scenarios.
const ROWS: usize = 3;

/// Options a frame scenario sets before drawing.
#[derive(Default, Deserialize)]
struct Options {
    /// Whether shrinking content clears the screen.
    #[serde(default)]
    clear: bool,
    /// Whether the mobile-height exception applies.
    #[serde(default)]
    mobile: bool,
    /// Whether the hardware cursor is shown.
    #[serde(default)]
    cursor: bool,
}

/// One change applied before a frame.
#[derive(Default, Deserialize)]
struct Step {
    /// Replacement content.
    lines: Option<Vec<String>>,
    /// New terminal width.
    columns: Option<usize>,
    /// New terminal height.
    rows: Option<usize>,
    /// Whether the frame is forced.
    #[serde(default)]
    force: bool,
}

/// What one frame wrote.
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Frame {
    /// Writes and cursor visibility escapes of the frame.
    writes: Vec<String>,
    /// Full redraws so far.
    full_redraws: usize,
}

/// A scenario: initial content, the steps that follow and the frames they must write.
#[derive(Deserialize)]
struct Scenario {
    /// Behavior the scenario belongs to.
    test: String,
    /// Content of the first frame.
    initial: Vec<String>,
    /// Changes producing the later frames.
    steps: Vec<Step>,
    /// Scenario options.
    options: Options,
    /// Expected frames, the first for the initial content.
    frames: Vec<Frame>,
}

/// The scenarios recorded for `test`.
fn scenarios(test: &str) -> Vec<Scenario> {
    let all: Vec<Scenario> = parse(include_str!("fixtures/frames.json"));
    let found: Vec<Scenario> = all.into_iter().filter(|case| case.test == test).collect();
    assert!(!found.is_empty(), "no scenario for {test}");
    found
}

/// Draws a scenario with a recording terminal and returns what each frame wrote.
fn draw(scenario: &Scenario) -> Vec<Frame> {
    let terminal = RecordingTerminal::new(COLUMNS, ROWS);
    let runtime = ManualRuntime::new();
    if scenario.options.mobile {
        runtime.set_environment("TERMUX_VERSION", "test");
    }
    let probe = Probe::shared(&[]);
    probe.borrow_mut().set_lines(&scenario.initial);
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        None,
    );
    tui.add_child(probe.clone());
    tui.set_clear_on_shrink(scenario.options.clear);
    succeeds(tui.set_show_hardware_cursor(scenario.options.cursor));
    let mut frames = Vec::new();
    for step in std::iter::once(&Step::default()).chain(&scenario.steps) {
        if let Some(lines) = &step.lines {
            probe.borrow_mut().set_lines(lines);
        }
        if step.columns.is_some() || step.rows.is_some() {
            let (columns, rows) = terminal.size();
            terminal.resize(step.columns.unwrap_or(columns), step.rows.unwrap_or(rows));
        }
        terminal.clear_writes();
        tui.request_render(step.force);
        succeeds(runtime.settle());
        frames.push(Frame {
            writes: terminal.writes(),
            full_redraws: tui.full_redraws(),
        });
    }
    frames
}

/// Asserts that every scenario of `test` writes its recorded frames.
fn assert_frames(test: &str) {
    for scenario in scenarios(test) {
        assert_eq!(draw(&scenario), scenario.frames, "{test}");
    }
}

#[test]
fn first_frame_preserves_scrollback() {
    assert_frames("first_frame_preserves_scrollback");
}

#[test]
fn unchanged_frame_only_repositions_cursor() {
    assert_frames("unchanged_frame_only_repositions_cursor");
}

#[test]
fn maestro_frames_renders_correctly_when_only_a_middle_line_changes_spinner_case() {
    assert_frames("middle_change_leaves_later_rows_untouched");
    let scene = Scene::new(40, 10);
    scene.start(&["Header", "Working...", "Footer"]);
    for frame in ["|", "/", "-", "\\"] {
        let working = format!("Working {frame}");
        scene.show(&["Header", &working, "Footer"]);
        assert_eq!(
            scene.viewport()[..3],
            ["Header", working.as_str(), "Footer"]
        );
    }
    scene.stop();
    scene.assert_recorded("spinner");
}

#[test]
fn maestro_frames_renders_correctly_when_first_line_changes_but_rest_stays_same() {
    assert_frames("first_change_retains_unchanged_suffix");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2", "Line 3"]);
    scene.show(&["CHANGED", "Line 1", "Line 2", "Line 3"]);
    assert_eq!(
        scene.viewport()[..4],
        ["CHANGED", "Line 1", "Line 2", "Line 3"]
    );
    scene.stop();
    scene.assert_recorded("first_line");
}

#[test]
fn maestro_frames_renders_correctly_when_last_line_changes_but_rest_stays_same() {
    assert_frames("last_change_keeps_prefix");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2", "Line 3"]);
    scene.show(&["Line 0", "Line 1", "Line 2", "CHANGED"]);
    assert_eq!(
        scene.viewport()[..4],
        ["Line 0", "Line 1", "Line 2", "CHANGED"]
    );
    scene.stop();
    scene.assert_recorded("last_line");
}

#[test]
fn maestro_frames_renders_correctly_when_multiple_non_adjacent_lines_change() {
    assert_frames("disjoint_changes_redraw_the_intervening_span");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2", "Line 3", "Line 4"]);
    scene.show(&["Line 0", "CHANGED 1", "Line 2", "CHANGED 3", "Line 4"]);
    assert_eq!(
        scene.viewport()[..5],
        ["Line 0", "CHANGED 1", "Line 2", "CHANGED 3", "Line 4"]
    );
    scene.stop();
    scene.assert_recorded("non_adjacent");
}

#[test]
fn append_at_bottom_scrolls_once() {
    assert_frames("append_at_bottom_scrolls_once");
}

#[test]
fn append_blank_line_is_not_lost() {
    assert_frames("append_blank_line_is_not_lost");
}

#[test]
fn maestro_frames_tracks_cursor_correctly_when_content_shrinks_with_unchanged_remaining_lines() {
    assert_frames("deleted_tail_clears_without_redrawing_prefix");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2", "Line 3", "Line 4"]);
    scene.show(&["Line 0", "Line 1", "Line 2"]);
    scene.show(&["Line 0", "CHANGED", "Line 2"]);
    assert_eq!(scene.viewport()[1], "CHANGED");
    scene.stop();
    scene.assert_recorded("shrink_cursor");
}

#[test]
fn default_empty_frame_clears_last_row() {
    assert_frames("default_empty_frame_clears_last_row");
    let scene = Scene::new(10, 3);
    scene.start(&["stale"]);
    assert_eq!(scene.viewport(), ["stale", "", ""]);
    scene.show(&[]);
    assert_eq!(scene.viewport(), ["", "", ""]);
}

#[test]
fn maestro_frames_handles_transition_from_content_to_empty_and_back_to_content() {
    assert_frames("empty_frame_can_grow_again");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2"]);
    assert_eq!(scene.viewport()[0], "Line 0");
    scene.show(&[]);
    assert!(scene.viewport().iter().all(String::is_empty));
    scene.show(&["New Line 0", "New Line 1"]);
    assert_eq!(scene.viewport()[..2], ["New Line 0", "New Line 1"]);
    scene.stop();
    scene.assert_recorded("empty_and_back");
}

#[test]
fn maestro_frames_triggers_full_re_render_when_terminal_width_changes() {
    assert_frames("width_change_clears_purges_and_redraws");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2"]);
    let initial = scene.tui.full_redraws();
    scene.resize(60, 10);
    assert!(scene.tui.full_redraws() > initial);
    scene.stop();
    scene.assert_recorded("width_change");
}

#[test]
fn maestro_frames_triggers_full_re_render_when_terminal_height_changes() {
    assert_frames("height_change_clears_purges_and_redraws");
    let scene = Scene::new(40, 10);
    scene.start(&["Line 0", "Line 1", "Line 2"]);
    let initial = scene.tui.full_redraws();
    scene.resize(40, 15);
    assert!(scene.tui.full_redraws() > initial);
    assert_eq!(scene.viewport()[..3], ["Line 0", "Line 1", "Line 2"]);
    scene.stop();
    scene.assert_recorded("height_change");
}

#[test]
fn maestro_frames_retains_mobile_height_resize_behavior() {
    assert_frames("mobile_height_change_keeps_history");
    let scene = Scene::new(40, 10);
    scene.runtime.set_environment("TERMUX_VERSION", "1");
    let lines: Vec<String> = (0..20).map(|row| format!("Line {row}")).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    scene.start(&lines);
    let initial = scene.tui.full_redraws();
    for height in [15, 8, 14, 11] {
        scene.resize(40, height);
    }
    assert_eq!(scene.tui.full_redraws(), initial);
    // The emulator shows other rows than a terminal does after a height change, so the
    // recorded bytes alone prove that no clear or purge was written.
    scene.stop();
    scene.assert_recorded("mobile_height");
}

#[test]
fn maestro_frames_clears_empty_rows_when_content_shrinks_significantly() {
    assert_frames("clear_on_shrink_resets_high_water");
    let scene = Scene::new(40, 10);
    scene.tui.set_clear_on_shrink(true);
    scene.start(&["Line 0", "Line 1", "Line 2", "Line 3", "Line 4", "Line 5"]);
    let initial = scene.tui.full_redraws();
    scene.show(&["Line 0", "Line 1"]);
    assert!(scene.tui.full_redraws() > initial);
    assert_eq!(scene.viewport()[..4], ["Line 0", "Line 1", "", ""]);
    scene.stop();
    scene.assert_recorded("shrink_clears_rows");
}

#[test]
fn maestro_frames_handles_shrink_to_single_line() {
    let scene = Scene::new(40, 10);
    scene.tui.set_clear_on_shrink(true);
    scene.start(&["Line 0", "Line 1", "Line 2", "Line 3"]);
    scene.show(&["Only line"]);
    assert_eq!(scene.viewport()[..2], ["Only line", ""]);
    scene.stop();
    scene.assert_recorded("shrink_single_line");
}

#[test]
fn maestro_frames_handles_shrink_to_empty() {
    let scene = Scene::new(40, 10);
    scene.tui.set_clear_on_shrink(true);
    scene.start(&["Line 0", "Line 1", "Line 2"]);
    scene.show(&[]);
    assert!(scene.viewport().iter().all(String::is_empty));
    scene.stop();
    scene.assert_recorded("shrink_to_empty");
}

#[test]
fn changed_scrollback_forces_full_redraw() {
    assert_frames("changed_scrollback_forces_full_redraw");
}

#[test]
fn maestro_frames_full_re_renders_when_deleted_lines_move_the_viewport_upward() {
    let scene = Scene::new(20, 5);
    let lines: Vec<String> = (0..12).map(|row| format!("Line {row}")).collect();
    scene.start(&lines.iter().map(String::as_str).collect::<Vec<_>>());
    let initial = scene.tui.full_redraws();
    scene.show(&lines[..7].iter().map(String::as_str).collect::<Vec<_>>());
    assert!(scene.tui.full_redraws() > initial);
    assert_eq!(
        scene.viewport(),
        ["Line 2", "Line 3", "Line 4", "Line 5", "Line 6"]
    );
    scene.stop();
    scene.assert_recorded("viewport_up");
}

#[test]
fn maestro_frames_appends_after_a_shrink_without_another_full_redraw_once_the_viewport_is_reset() {
    assert_frames("shrink_above_viewport_resets_then_appends");
    let scene = Scene::new(20, 5);
    let lines: Vec<String> = (0..8).map(|row| format!("Line {row}")).collect();
    scene.start(&lines.iter().map(String::as_str).collect::<Vec<_>>());
    let initial = scene.tui.full_redraws();
    scene.show(&["Line 0", "Line 1"]);
    let after_shrink = scene.tui.full_redraws();
    assert!(after_shrink > initial);
    scene.show(&["Line 0", "Line 1", "Line 2"]);
    assert_eq!(scene.tui.full_redraws(), after_shrink);
    assert_eq!(scene.viewport(), ["Line 0", "Line 1", "Line 2", "", ""]);
    scene.stop();
    scene.assert_recorded("append_after_shrink");
}

#[test]
fn maestro_frames_clears_stale_content_when_maxlinesrendered_was_inflated_by_a_transient_component()
{
    let scene = Scene::new(40, 10);
    let editor = Probe::shared(&["Editor 0", "Editor 1", "Editor 2"]);
    scene.tui.add_child(editor.clone());
    let chat =
        |count: usize| -> Vec<String> { (0..count).map(|row| format!("Chat {row}")).collect() };
    let selector: Vec<String> = (0..8).map(|row| format!("Selector {row}")).collect();
    let editor_lines: Vec<String> = (0..3).map(|row| format!("Editor {row}")).collect();
    scene.probe.borrow_mut().set_lines(&chat(15));
    scene.tui.start().unwrap();
    scene.render();
    editor.borrow_mut().set_lines(&selector);
    scene.render();
    editor.borrow_mut().set_lines(&editor_lines);
    scene.render();
    let before_switch = scene.tui.full_redraws();
    scene.probe.borrow_mut().set_lines(&chat(12));
    scene.render();
    assert!(scene.tui.full_redraws() > before_switch);
    let viewport = scene.viewport();
    assert!(
        viewport
            .iter()
            .all(|row| !["Chat 12", "Chat 13", "Chat 14"].contains(&row.as_str()))
    );
    assert_eq!(
        viewport,
        [
            "Chat 5", "Chat 6", "Chat 7", "Chat 8", "Chat 9", "Chat 10", "Chat 11", "Editor 0",
            "Editor 1", "Editor 2"
        ]
    );
    scene.stop();
    scene.assert_recorded("transient_component");
    assert_eq!(scene.tui.children().len(), 2);
    let editor: ComponentHandle = editor;
    scene.tui.remove_child(&editor);
    assert_eq!(scene.tui.children().len(), 1);
    let mut nested = scene.tui.clone();
    assert_eq!(
        nested.render(40),
        (0..12).map(|row| format!("Chat {row}")).collect::<Vec<_>>()
    );
    nested.invalidate();
    assert_eq!(scene.probe.borrow().invalidated, 1);
    scene.tui.clear();
    assert!(scene.tui.children().is_empty());
    assert_eq!(scene.tui.terminal().borrow().columns(), 40);
}

/// A graphics line and the image ids a redraw must delete for it.
#[derive(Deserialize)]
struct ImageLine {
    /// Behavior the line belongs to.
    test: String,
    /// The component line.
    line: String,
    /// Ids the writer deletes before the next full redraw, in order.
    ids: Vec<u32>,
}

/// Draws each recorded line, forces a redraw of other content and asserts which images
/// the redraw deletes.
fn assert_deleted_images(test: &str) {
    let lines: Vec<ImageLine> = parse(include_str!("fixtures/image_ids.json"));
    for case in lines.iter().filter(|case| case.test == test) {
        let scenario = Scenario {
            test: test.to_owned(),
            initial: vec![case.line.clone()],
            steps: vec![Step {
                lines: Some(vec!["x".to_owned()]),
                force: true,
                ..Step::default()
            }],
            options: Options::default(),
            frames: Vec::new(),
        };
        let deleted: String = case.ids.iter().map(|id| delete_kitty_image(*id)).collect();
        let frames = draw(&scenario);
        assert_eq!(
            frames[1].writes[0],
            format!("\x1b[?2026h{deleted}\x1b[2J\x1b[H\x1b[3Jx\x1b[0m\x1b]8;;\x07\x1b[?2026l"),
            "{:?}",
            case.line
        );
    }
}

#[test]
fn image_ids_obey_numeric_boundaries() {
    assert_deleted_images("image_ids_obey_numeric_boundaries");
}

#[test]
fn image_headers_preserve_first_sequence_selection() {
    assert_deleted_images("image_headers_preserve_first_sequence_selection");
}

#[test]
fn duplicate_image_ids_are_deleted_once() {
    assert_frames("duplicate_image_ids_are_deleted_once");
}

#[test]
fn image_change_ranges_preserve_deletion_order() {
    assert_frames("image_change_ranges_preserve_deletion_order");
}

/// A Kitty placement of `payload` as the components render it.
fn placement(payload: &str, rows: usize, image_id: u32) -> String {
    encode_kitty(
        payload,
        KittyOptions {
            columns: Some(2),
            rows: Some(rows),
            image_id: Some(image_id),
            move_cursor: false,
        },
    )
}

#[test]
fn maestro_frames_deletes_changed_image_ids_before_drawing_moved_placements() {
    assert_frames("changed_image_is_deleted_before_placement");
    let scene = Scene::new(40, 10);
    scene.start(&["top", &placement("AAAA", 2, 42)]);
    let mark = scene.terminal.writes().len();
    let moved = placement("BBBB", 1, 42);
    scene.show(&[&moved, ""]);
    let writes = scene.terminal.writes()[mark..].concat();
    let deleted = writes.find(&delete_kitty_image(42)).unwrap();
    let drawn = writes.find(&moved).unwrap();
    assert!(
        deleted < drawn,
        "the old placement is deleted before the new one is drawn"
    );
    scene.stop();
    scene.assert_recorded("changed_image_ids");
}

#[test]
fn maestro_frames_redraws_image_lines_when_an_earlier_reserved_image_row_changes() {
    assert_frames("reserved_row_change_redraws_image");
    let scene = Scene::new(40, 10);
    let image = placement("AAAA", 2, 88);
    scene.start(&["", &image]);
    let mark = scene.terminal.writes().len();
    scene.show(&["covered", &image]);
    let writes = scene.terminal.writes()[mark..].concat();
    let deleted = writes.find(&delete_kitty_image(88)).unwrap();
    let drawn = writes.find(&image).unwrap();
    assert!(
        deleted < drawn,
        "the old placement is deleted before the line is drawn again"
    );
    assert!(
        !writes.contains("\x1b[2J"),
        "a reserved row change is not a full redraw"
    );
    scene.stop();
    scene.assert_recorded("reserved_image_row");
}

#[test]
fn maestro_frames_deletes_previously_rendered_image_ids_during_full_redraws() {
    assert_frames("forced_redraw_deletes_prior_images");
    let scene = Scene::new(40, 10);
    scene.start(&[&placement("AAAA", 2, 77)]);
    let mark = scene.terminal.writes().len();
    scene
        .probe
        .borrow_mut()
        .set_lines(&["plain text".to_owned()]);
    scene.tui.request_render(true);
    scene.runtime.settle().unwrap();
    let writes = scene.terminal.writes()[mark..].concat();
    let deleted = writes.find(&delete_kitty_image(77)).unwrap();
    let cleared = writes.find("\x1b[2J").unwrap();
    assert!(
        deleted < cleared,
        "the old image is deleted before the screen is cleared"
    );
    scene.stop();
    scene.assert_recorded("full_redraw_images");
}

/// Component lines and the lines a frame holds once resets are applied.
#[derive(Deserialize)]
struct LineReset {
    /// Lines a component renders.
    lines: Vec<String>,
    /// Lines as the first frame draws them.
    expected: Vec<String>,
}

#[test]
fn maestro_frames_resets_styles_after_each_rendered_line() {
    let cases: Vec<LineReset> = parse(include_str!("fixtures/line_resets.json"));
    for case in cases {
        let scenario = Scenario {
            test: String::new(),
            initial: case.lines.clone(),
            steps: Vec::new(),
            options: Options::default(),
            frames: Vec::new(),
        };
        let frames = draw(&scenario);
        let expected = format!("\x1b[?2026h{}\x1b[?2026l", case.expected.join("\r\n"));
        assert_eq!(frames[0].writes[0], expected, "{:?}", case.lines);
    }
    let scene = Scene::new(20, 6);
    scene.start(&["\x1b[3mItalic", "Plain"]);
    assert!(!scene.terminal.is_italic(1, 0));
    scene.stop();
    scene.assert_recorded("style_resets");
}

/// Component lines with markers and the first frame the writer draws for them.
#[derive(Deserialize)]
struct MarkedLines {
    /// Lines a component renders, markers included.
    lines: Vec<String>,
    /// Writes of the first frame with the hardware cursor enabled.
    writes: Vec<String>,
}

#[test]
fn cursor_markers_choose_last_visible_row() {
    let cases: Vec<MarkedLines> = parse(include_str!("fixtures/cursor_markers.json"));
    for case in cases {
        let scenario = Scenario {
            test: String::new(),
            initial: case.lines.clone(),
            steps: Vec::new(),
            options: Options {
                cursor: true,
                ..Options::default()
            },
            frames: Vec::new(),
        };
        assert_eq!(draw(&scenario)[0].writes, case.writes, "{:?}", case.lines);
    }
}

#[test]
fn cursor_only_move_skips_frame_drawing() {
    assert_frames("cursor_only_move_skips_frame_drawing");
}

#[test]
fn cursor_row_motion_keeps_logical_end() {
    assert_frames("cursor_row_motion_keeps_logical_end");
}
