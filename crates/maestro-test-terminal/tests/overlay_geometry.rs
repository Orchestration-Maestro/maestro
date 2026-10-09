//! Overlay placement and composition observed as exact terminal writes.

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

use maestro_tui::{OverlayOptions, SizeValue, TUI, TerminalImage};
use std::cell::RefCell;
use std::rc::Rc;
use support::{
    components::Probe, manual_runtime::ManualRuntime, recording_terminal::RecordingTerminal,
};

#[test]
fn maestro_frames_render_overlay_when_content_is_shorter_than_terminal_height() {
    corpus("maestro_frames_render_overlay_when_content_is_shorter_than_terminal_height").unwrap();
}

/// One raw public query and its recorded terminal effects.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    /// Diagnostic name, not an alternate query identity.
    case: String,
    /// All public operands; also used to reject repeated queries.
    input: serde_json::Value,
    /// Recorded effects.
    expected: Expected,
}

/// Expected observations through the public writer.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    /// Exact emitted bytes.
    writes: Vec<String>,
    /// Width for each render.
    requested_widths: Vec<usize>,
}

/// The authored test operands, not a product wire format.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Query {
    /// Terminal width.
    columns: usize,
    /// Terminal height.
    rows: usize,
    /// Base lines.
    base: Vec<String>,
    /// Live overlay options.
    options: FixtureOptions,
    /// Overlay lines.
    lines: Vec<String>,
}

/// Authored fixture size union.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum FixtureSize {
    /// Signed cells.
    Cells(isize),
    /// Percentage spelling.
    Percentage(String),
}
impl From<FixtureSize> for SizeValue {
    fn from(value: FixtureSize) -> Self {
        match value {
            FixtureSize::Cells(value) => Self::Cells(value),
            FixtureSize::Percentage(value) => Self::Percentage(value),
        }
    }
}

/// Authored margin union.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum FixtureMargin {
    /// Scalar margin.
    Uniform(isize),
    /// Per-edge margin.
    Sides {
        top: Option<isize>,
        right: Option<isize>,
        bottom: Option<isize>,
        left: Option<isize>,
    },
}

/// Authored anchor spelling.
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum FixtureAnchor {
    /// Center.
    Center,
    /// Top left.
    TopLeft,
    /// Top right.
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    BottomRight,
    /// Top center.
    TopCenter,
    /// Bottom center.
    BottomCenter,
    /// Left center.
    LeftCenter,
    /// Right center.
    RightCenter,
}

/// Options used by the byte corpus.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FixtureOptions {
    /// Rendering width.
    width: Option<FixtureSize>,
    /// Minimum rendering width.
    min_width: Option<isize>,
    /// Truncation height.
    max_height: Option<FixtureSize>,
    /// Anchor spelling.
    anchor: Option<FixtureAnchor>,
    /// Horizontal offset.
    offset_x: Option<isize>,
    /// Vertical offset.
    offset_y: Option<isize>,
    /// Explicit row.
    row: Option<FixtureSize>,
    /// Explicit column.
    col: Option<FixtureSize>,
    /// Edge margins.
    margin: Option<FixtureMargin>,
}

impl From<FixtureOptions> for OverlayOptions {
    fn from(value: FixtureOptions) -> Self {
        use maestro_tui::tui::{OverlayAnchor as A, OverlayMargin, OverlayMarginValue as M};
        Self {
            width: value.width.map(Into::into),
            min_width: value.min_width,
            max_height: value.max_height.map(Into::into),
            anchor: value.anchor.map(|anchor| match anchor {
                FixtureAnchor::Center => A::Center,
                FixtureAnchor::TopLeft => A::TopLeft,
                FixtureAnchor::TopRight => A::TopRight,
                FixtureAnchor::BottomLeft => A::BottomLeft,
                FixtureAnchor::BottomRight => A::BottomRight,
                FixtureAnchor::TopCenter => A::TopCenter,
                FixtureAnchor::BottomCenter => A::BottomCenter,
                FixtureAnchor::LeftCenter => A::LeftCenter,
                FixtureAnchor::RightCenter => A::RightCenter,
            }),
            offset_x: value.offset_x,
            offset_y: value.offset_y,
            row: value.row.map(Into::into),
            col: value.col.map(Into::into),
            margin: value.margin.map(|margin| match margin {
                FixtureMargin::Uniform(value) => M::Uniform(value),
                FixtureMargin::Sides {
                    top,
                    right,
                    bottom,
                    left,
                } => M::Sides(OverlayMargin {
                    top,
                    right,
                    bottom,
                    left,
                }),
            }),
            ..Self::default()
        }
    }
}

/// Checks unique public queries and consumes every committed operand and expectation.
fn corpus(group: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::{BTreeMap, HashSet};
    let mut corpus: BTreeMap<String, Vec<Fixture>> =
        serde_json::from_str(include_str!("fixtures/overlays.json"))?;
    let mut queries = HashSet::new();
    for fixture in corpus.values().flatten() {
        assert!(
            queries.insert(fixture.input.to_string()),
            "repeated query {}",
            fixture.case
        );
    }
    let Some(fixtures) = corpus.remove(group) else {
        return Err(format!("missing group {group}").into());
    };
    for fixture in fixtures {
        let query: Query = serde_json::from_value(fixture.input)?;
        let terminal = RecordingTerminal::new(query.columns, query.rows);
        let runtime = ManualRuntime::new();
        let tui = TUI::new(
            terminal.handle(),
            runtime.handle(),
            TerminalImage::new(|_| None, || 1),
            None,
        );
        let base = Probe::shared(&[]);
        base.set_lines(&query.base);
        tui.add_child(base);
        let overlay = Probe::shared(&[]);
        overlay.set_lines(&query.lines);
        tui.show_overlay(
            overlay.clone(),
            Some(Rc::new(RefCell::new(query.options.into()))),
        )?;
        tui.start()?;
        terminal.clear_writes();
        runtime.settle()?;
        assert_eq!(
            terminal.writes(),
            fixture.expected.writes,
            "{}",
            fixture.case
        );
        assert_eq!(
            *overlay.widths.borrow(),
            fixture.expected.requested_widths,
            "{}",
            fixture.case
        );
        tui.stop()?;
    }
    Ok(())
}

#[test]
fn overlay_size_defaults_and_signed_bounds() {
    corpus("overlay_size_defaults_and_signed_bounds").unwrap();
}

#[test]
fn overlay_all_anchors_respect_asymmetric_margins() {
    corpus("overlay_all_anchors_respect_asymmetric_margins").unwrap();
}

#[test]
fn overlay_percentage_grammar_is_exact() {
    corpus("overlay_percentage_grammar_is_exact").unwrap();
}

#[test]
fn overlay_margins_offsets_and_precedence_match_layout() {
    corpus("overlay_margins_offsets_and_precedence_match_layout").unwrap();
}

#[test]
fn overlay_composition_keeps_segment_styles_and_cell_boundaries() {
    corpus("overlay_composition_keeps_segment_styles_and_cell_boundaries").unwrap();
}

#[test]
fn overlay_composition_expands_tabs_before_selecting_columns() {
    corpus("overlay_composition_expands_tabs_before_selecting_columns").unwrap();
}

#[test]
fn overlay_height_and_viewport_padding_keep_screen_positions() {
    corpus("overlay_height_and_viewport_padding_keep_screen_positions").unwrap();
}

#[test]
fn maestro_overlays_truncate_overlay_lines_that_exceed_declared_width() {
    corpus("maestro_overlays_truncate_overlay_lines_that_exceed_declared_width").unwrap();
}

#[test]
fn maestro_overlays_handle_overlay_with_complex_ansi_sequences_without_crashing() {
    corpus("maestro_overlays_handle_overlay_with_complex_ansi_sequences_without_crashing").unwrap();
}

#[test]
fn maestro_overlays_handle_overlay_composited_on_styled_base_content() {
    corpus("maestro_overlays_handle_overlay_composited_on_styled_base_content").unwrap();
}

#[test]
fn maestro_overlays_handle_wide_characters_at_overlay_boundary() {
    corpus("maestro_overlays_handle_wide_characters_at_overlay_boundary").unwrap();
}

#[test]
fn maestro_overlays_handle_overlay_positioned_at_terminal_edge() {
    corpus("maestro_overlays_handle_overlay_positioned_at_terminal_edge").unwrap();
}

#[test]
fn maestro_overlays_handle_overlay_on_base_content_with_osc_sequences() {
    corpus("maestro_overlays_handle_overlay_on_base_content_with_osc_sequences").unwrap();
}

#[test]
fn maestro_overlays_render_overlay_at_percentage_of_terminal_width() {
    corpus("maestro_overlays_render_overlay_at_percentage_of_terminal_width").unwrap();
}

#[test]
fn maestro_overlays_respect_minwidth_when_widthpercent_results_in_smaller_width() {
    corpus("maestro_overlays_respect_minwidth_when_widthpercent_results_in_smaller_width").unwrap();
}

#[test]
fn maestro_overlays_position_overlay_at_top_left() {
    corpus("maestro_overlays_position_overlay_at_top_left").unwrap();
}

#[test]
fn maestro_overlays_position_overlay_at_bottom_right() {
    corpus("maestro_overlays_position_overlay_at_bottom_right").unwrap();
}

#[test]
fn maestro_overlays_position_overlay_at_top_center() {
    corpus("maestro_overlays_position_overlay_at_top_center").unwrap();
}

#[test]
fn maestro_overlays_clamp_negative_margins_to_zero() {
    corpus("maestro_overlays_clamp_negative_margins_to_zero").unwrap();
}

#[test]
fn maestro_overlays_respect_margin_as_number() {
    corpus("maestro_overlays_respect_margin_as_number").unwrap();
}

#[test]
fn maestro_overlays_respect_margin_object() {
    corpus("maestro_overlays_respect_margin_object").unwrap();
}

#[test]
fn maestro_overlays_apply_offsetx_and_offsety_from_anchor_position() {
    corpus("maestro_overlays_apply_offsetx_and_offsety_from_anchor_position").unwrap();
}

#[test]
fn maestro_overlays_position_with_rowpercent_and_colpercent() {
    corpus("maestro_overlays_position_with_rowpercent_and_colpercent").unwrap();
}

#[test]
fn maestro_overlays_rowpercent_0_should_position_at_top() {
    corpus("maestro_overlays_rowpercent_0_should_position_at_top").unwrap();
}

#[test]
fn maestro_overlays_rowpercent_100_should_position_at_bottom() {
    corpus("maestro_overlays_rowpercent_100_should_position_at_bottom").unwrap();
}

#[test]
fn maestro_overlays_truncate_overlay_to_maxheight() {
    corpus("maestro_overlays_truncate_overlay_to_maxheight").unwrap();
}

#[test]
fn maestro_overlays_truncate_overlay_to_maxheightpercent() {
    corpus("maestro_overlays_truncate_overlay_to_maxheightpercent").unwrap();
}

#[test]
fn maestro_overlays_row_and_col_should_override_anchor() {
    corpus("maestro_overlays_row_and_col_should_override_anchor").unwrap();
}

#[test]
fn maestro_frames_not_leak_styles_when_overlay_slicing_drops_trailing_sgr_resets() {
    let scene = support::scene::Scene::new(20, 6);
    let base = format!("\x1b[3m{}\x1b[23m", "X".repeat(20));
    scene.probe.set_lines(&[base, "INPUT".to_owned()]);
    scene
        .tui
        .show_overlay(
            Probe::shared(&["OVR"]),
            Some(Rc::new(RefCell::new(OverlayOptions {
                width: Some(SizeValue::Cells(3)),
                row: Some(SizeValue::Cells(0)),
                col: Some(SizeValue::Cells(5)),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    scene.tui.start().unwrap();
    scene.render();
    assert!(!scene.terminal.is_italic(1, 0));
    assert!(scene.terminal.writes().iter().any(|write| {
        write.contains("\x1b[3mXXXXX\x1b[0m\x1b]8;;\x07OVR\x1b[0m\x1b]8;;\x07\x1b[3m")
    }));
    scene.stop();
}

#[test]
fn maestro_overlays_render_multiple_overlays_with_later_ones_on_top() {
    use maestro_tui::OverlayAnchor;
    let scene = support::scene::Scene::new(80, 24);
    scene.start(&[]);
    scene
        .tui
        .show_overlay(
            Probe::shared(&["FIRST-OVERLAY"]),
            Some(Rc::new(RefCell::new(OverlayOptions {
                anchor: Some(OverlayAnchor::TopLeft),
                width: Some(SizeValue::Cells(20)),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    scene
        .tui
        .show_overlay(
            Probe::shared(&["SECOND"]),
            Some(Rc::new(RefCell::new(OverlayOptions {
                anchor: Some(OverlayAnchor::TopLeft),
                width: Some(SizeValue::Cells(10)),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    scene.render();
    assert_eq!(scene.viewport()[0], "SECOND    LAY");
    assert!(
        scene
            .terminal
            .writes()
            .iter()
            .any(|write| write.contains("SECOND    \x1b[0m\x1b]8;;\x07LAY"))
    );
    scene.stop();
}

#[test]
fn maestro_overlays_handle_overlays_at_different_positions_without_interference() {
    use maestro_tui::OverlayAnchor;
    let scene = support::scene::Scene::new(80, 24);
    scene.start(&[]);
    for (label, anchor) in [
        ("TOP-LEFT", OverlayAnchor::TopLeft),
        ("BTM-RIGHT", OverlayAnchor::BottomRight),
    ] {
        scene
            .tui
            .show_overlay(
                Probe::shared(&[label]),
                Some(Rc::new(RefCell::new(OverlayOptions {
                    anchor: Some(anchor),
                    width: Some(SizeValue::Cells(15)),
                    ..OverlayOptions::default()
                }))),
            )
            .unwrap();
    }
    scene.render();
    assert_eq!(scene.viewport()[0], "TOP-LEFT");
    assert_eq!(scene.viewport()[23], format!("{}BTM-RIGHT", " ".repeat(65)));
    let writes = scene.terminal.writes().concat();
    assert!(writes.contains("TOP-LEFT       \x1b[0m\x1b]8;;\x07"));
    assert!(writes.contains("BTM-RIGHT      \x1b[0m\x1b]8;;\x07"));
    scene.stop();
}

#[test]
fn maestro_overlays_properly_hide_overlays_in_stack_order() {
    use maestro_tui::OverlayAnchor;
    let scene = support::scene::Scene::new(80, 24);
    scene.start(&[]);
    for label in ["FIRST", "SECOND"] {
        scene
            .tui
            .show_overlay(
                Probe::shared(&[label]),
                Some(Rc::new(RefCell::new(OverlayOptions {
                    anchor: Some(OverlayAnchor::TopLeft),
                    width: Some(SizeValue::Cells(10)),
                    ..OverlayOptions::default()
                }))),
            )
            .unwrap();
    }
    scene.render();
    assert_eq!(scene.viewport()[0], "SECOND");
    scene.tui.hide_overlay().unwrap();
    let before = scene.terminal.writes().len();
    scene.render();
    assert_eq!(scene.viewport()[0], "FIRST");
    assert!(
        scene.terminal.writes()[before..]
            .concat()
            .contains("FIRST     \x1b[0m\x1b]8;;\x07")
    );
    scene.stop();
}

#[test]
fn overlay_size_percentages_multiply_before_dividing() {
    let scene = support::scene::Scene::new(100, 100);
    scene.start(&[]);
    let overlay = Probe::shared(&[]);
    overlay.set_lines(&(0..100).map(|_| "X".to_owned()).collect::<Vec<_>>());
    scene
        .tui
        .show_overlay(
            overlay.clone(),
            Some(Rc::new(RefCell::new(OverlayOptions {
                width: Some(SizeValue::Percentage("58%".to_owned())),
                max_height: Some(SizeValue::Percentage("58%".to_owned())),
                row: Some(SizeValue::Cells(0)),
                col: Some(SizeValue::Cells(0)),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    scene.render();
    assert_eq!(*overlay.widths.borrow(), [58]);
    assert_eq!(
        scene
            .viewport()
            .iter()
            .filter(|line| line.starts_with('X'))
            .count(),
        58
    );
    scene.stop();
}

#[test]
fn overlay_tab_expansion_keeps_unterminated_escape_text_and_clips_it() {
    for (line, width, expected) in [
        ("\x1b]\tZ".to_owned(), 6, "\x1b]   Z"),
        (
            format!("{}\t", "\x1b]".repeat(100_000)),
            4,
            "\x1b]\x1b]\x1b]\x1b]",
        ),
    ] {
        let scene = support::scene::Scene::new(10, 2);
        scene.start(&[]);
        let overlay = Probe::shared(&[]);
        overlay.set_lines(&[line]);
        scene
            .tui
            .show_overlay(
                overlay,
                Some(Rc::new(RefCell::new(OverlayOptions {
                    width: Some(SizeValue::Cells(width)),
                    row: Some(SizeValue::Cells(0)),
                    col: Some(SizeValue::Cells(0)),
                    ..OverlayOptions::default()
                }))),
            )
            .unwrap();
        scene.render();
        assert!(
            scene
                .terminal
                .writes()
                .iter()
                .any(|write| write.contains(expected))
        );
        assert!(
            scene
                .terminal
                .writes()
                .iter()
                .all(|write| !write.contains('\t'))
        );
        scene.stop();
    }
}

#[test]
fn overlay_percentage_coordinates_keep_precision_until_final_clamp() {
    let scene = support::scene::Scene::new(21, 21);
    scene.start(&[]);
    scene
        .tui
        .show_overlay(
            Probe::shared(&["X"]),
            Some(Rc::new(RefCell::new(OverlayOptions {
                width: Some(SizeValue::Cells(1)),
                row: Some(SizeValue::Percentage("100000000000000000000%".to_owned())),
                col: Some(SizeValue::Percentage("100000000000000000000%".to_owned())),
                offset_x: Some(isize::MIN),
                offset_y: Some(isize::MIN),
                ..OverlayOptions::default()
            }))),
        )
        .unwrap();
    scene.render();
    let line = format!("{}\x1b[0m\x1b]8;;\x07X\x1b[0m\x1b]8;;\x07", " ".repeat(20));
    assert!(scene.terminal.writes().concat().contains(&line));
    assert_eq!(scene.viewport()[20], format!("{}X", " ".repeat(20)));
    assert!(
        scene.viewport()[..20]
            .iter()
            .all(|line| !line.contains('X'))
    );
    scene.stop();
}
