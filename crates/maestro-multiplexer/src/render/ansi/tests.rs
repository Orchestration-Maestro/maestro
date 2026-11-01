use super::style::{Layer, build_sgr, color_to_sgr, modifier_to_sgr_parts};
use super::*;
use maestro_protocol::frames::{CellData, CursorState, with_underline_style};
use serde::Deserialize;

const WIDE_GRAPHEME: &str = "💡";
const HALFWIDTH_VOICED_KANA: &str = "ｶ\u{ff9e}";
const SYNC_START_AND_HIDE: &str = "\x1b[?2026h\x1b[?25l";
const PROLOGUE: &str = "\x1b[?2026h\x1b[?25l\x1b]8;;\x1b\\";

fn make_cell(symbol: &str, fg: u32, bg: u32, modifier: u16) -> CellData {
    CellData {
        symbol: symbol.to_owned(),
        fg,
        bg,
        modifier,
        skip: false,
        hyperlink: None,
    }
}

fn plain(symbol: &str) -> CellData {
    make_cell(symbol, 0, 0, 0)
}

fn skip(symbol: &str) -> CellData {
    CellData {
        skip: true,
        ..plain(symbol)
    }
}

fn linked(symbol: &str, index: u32) -> CellData {
    CellData {
        hyperlink: Some(index),
        ..plain(symbol)
    }
}

fn make_frame(width: u16, height: u16, cells: Vec<CellData>) -> FrameData {
    FrameData {
        cells,
        width,
        height,
        cursor: None,
        hyperlinks: Vec::new(),
        graphics: Vec::new(),
    }
}

fn with_cursor(mut frame: FrameData, x: u16, y: u16, visible: bool, shape: u8) -> FrameData {
    frame.cursor = Some(CursorState {
        x,
        y,
        visible,
        shape,
    });
    frame
}

fn hi_frame() -> FrameData {
    make_frame(2, 2, vec![plain("H"), plain("i"), plain("!"), plain(" ")])
}

fn policy(repeat_ime_anchor: bool) -> BlitPolicy {
    BlitPolicy {
        repeat_ime_anchor,
        clear_before_full_redraw: true,
        suppress_visible_cursor: false,
    }
}

fn blit_with(
    frame: &FrameData,
    previous: Option<&FrameData>,
    memory: &mut CursorMemory,
    policy: BlitPolicy,
) -> String {
    String::from_utf8(blit_frame(frame, previous, memory, policy)).unwrap()
}

/// Encode with fresh cursor memory and this platform's anchor policy.
fn blit(frame: &FrameData, previous: Option<&FrameData>) -> String {
    blit_with(
        frame,
        previous,
        &mut CursorMemory::default(),
        policy(!cfg!(windows)),
    )
}

/// Split encoded output into the bytes before and after synchronized output ends.
fn split_at_sync_end(output: &str) -> (&str, &str) {
    output.split_once(SYNC_OUTPUT_END).unwrap()
}

#[test]
fn encoder_named_foreground_colors_map_to_ansi() {
    assert_eq!(color_to_sgr(0x00_00_00_00, Layer::Foreground), "39");
    assert_eq!(color_to_sgr(0x00_00_00_01, Layer::Foreground), "30");
    assert_eq!(color_to_sgr(0x00_00_00_02, Layer::Foreground), "31");
    assert_eq!(color_to_sgr(0x00_00_00_10, Layer::Foreground), "97");
}

#[test]
fn encoder_indexed_foreground_color_maps_to_256_color() {
    assert_eq!(color_to_sgr(0x01_00_00_AB, Layer::Foreground), "38;5;171");
}

#[test]
fn encoder_rgb_foreground_color_maps_to_true_color() {
    assert_eq!(
        color_to_sgr(0x02_FF_80_40, Layer::Foreground),
        "38;2;255;128;64"
    );
}

#[test]
fn encoder_named_background_colors_map_to_ansi() {
    assert_eq!(color_to_sgr(0x00_00_00_00, Layer::Background), "49");
    assert_eq!(color_to_sgr(0x00_00_00_01, Layer::Background), "40");
    assert_eq!(color_to_sgr(0x00_00_00_10, Layer::Background), "107");
}

#[test]
fn encoder_rgb_background_color_maps_to_true_color() {
    assert_eq!(
        color_to_sgr(0x02_FF_80_40, Layer::Background),
        "48;2;255;128;64"
    );
}

#[test]
fn encoder_unknown_colors_select_the_default() {
    assert_eq!(color_to_sgr(0x00_00_00_11, Layer::Foreground), "39");
    assert_eq!(color_to_sgr(0x03_00_00_01, Layer::Foreground), "39");
    assert_eq!(color_to_sgr(0x00_00_00_09, Layer::Background), "100");
    assert_eq!(color_to_sgr(0xFF_00_00_01, Layer::Background), "49");
}

#[test]
fn encoder_bold_modifier_emits_bold() {
    assert!(modifier_to_sgr_parts(1).contains(&"1"));
}

#[test]
fn encoder_italic_modifier_emits_italic() {
    assert!(modifier_to_sgr_parts(4).contains(&"3"));
}

#[test]
fn encoder_plain_modifiers_emit_no_parts() {
    assert!(modifier_to_sgr_parts(0).is_empty());
}

#[test]
fn encoder_underline_styles_use_colon_parameters() {
    let underlined = Modifier::UNDERLINED.bits();
    let parts: Vec<_> = (0..8)
        .map(|style| modifier_to_sgr_parts(with_underline_style(underlined, style)))
        .collect();
    assert_eq!(
        parts,
        [
            ["4"],
            ["4"],
            ["4:2"],
            ["4:3"],
            ["4:4"],
            ["4:5"],
            ["4"],
            ["4"]
        ]
    );
    assert!(modifier_to_sgr_parts(with_underline_style(0, 3)).is_empty());
}

#[test]
fn encoder_style_sequence_wraps_reset_and_colors() {
    assert_eq!(
        build_sgr(0x00_00_00_02, 0x00_00_00_01, 1),
        "\x1b[0;1;31;40m"
    );
}

#[test]
fn encoder_plain_cell_resets_previous_modifiers() {
    assert_eq!(build_sgr(0, 0, 0), "\x1b[0;39;49m");
}

#[test]
fn encoder_curly_underline_style_survives_style_sequence() {
    let modifier = with_underline_style(Modifier::UNDERLINED.bits(), 3);
    assert_eq!(build_sgr(0, 0, modifier), "\x1b[0;4:3;39;49m");
}

#[test]
fn encoder_equal_cells_compare_visually_equal() {
    let cell = make_cell("A", 2, 1, 0);
    assert!(redraw::cells_visually_equal(&[], &cell, &[], &cell.clone()));
}

#[test]
fn encoder_cells_with_different_symbols_differ() {
    let (a, b) = (make_cell("A", 2, 1, 0), make_cell("B", 2, 1, 0));
    assert!(!redraw::cells_visually_equal(&[], &a, &[], &b));
}

#[test]
fn encoder_cells_with_different_colors_differ() {
    let (a, b) = (make_cell("A", 2, 1, 0), make_cell("A", 3, 1, 0));
    assert!(!redraw::cells_visually_equal(&[], &a, &[], &b));
}

#[test]
fn encoder_cells_compare_sanitized_link_targets() {
    let links = [Some("https://a.test".to_owned())];
    let other = [Some("https://b.test".to_owned())];
    let cell = linked("L", 0);
    assert!(redraw::cells_visually_equal(
        &links,
        &cell,
        &links.clone(),
        &cell
    ));
    assert!(!redraw::cells_visually_equal(&links, &cell, &other, &cell));
    assert!(!redraw::cells_visually_equal(
        &links,
        &cell,
        &links,
        &plain("L")
    ));
}

#[test]
fn encoder_full_redraw_hides_cursor_before_cells() {
    assert!(blit(&hi_frame(), None).starts_with(SYNC_START_AND_HIDE));
}

#[test]
fn encoder_diff_hides_cursor_before_cells() {
    let mut current = hi_frame();
    current.cells[0] = plain("X");
    assert!(blit(&current, Some(&hi_frame())).starts_with(SYNC_START_AND_HIDE));
}

#[test]
fn encoder_wraps_frame_in_synchronized_output() {
    let output = blit(&make_frame(1, 1, vec![plain("A")]), None);
    assert!(output.starts_with(SYNC_START_AND_HIDE));
    assert!(output.find(SYNC_OUTPUT_END).unwrap() > 0);
}

#[test]
fn encoder_restarts_sync_before_hiding_cursor_on_next_frame() {
    let visible = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 2, 1, true, 0);
    let mut changed = visible.clone();
    changed.cells[0] = plain("B");
    let mut memory = CursorMemory::default();
    let first = blit_with(&visible, None, &mut memory, policy(true));
    let second = blit_with(&changed, Some(&visible), &mut memory, policy(true));

    assert!(second.starts_with(SYNC_START_AND_HIDE));
    assert!(second.find("\x1b[?25l").unwrap() < second.find("\x1b[1;1H").unwrap());
    assert!(
        format!("{first}{second}").contains("\x1b[?2026l\x1b[2;3H\x1b[?25h\x1b[?2026h\x1b[?25l")
    );
}

#[test]
fn encoder_repeats_cursor_anchor_after_sync_when_enabled() {
    let frame = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 2, 1, true, 0);
    let output = blit_with(&frame, None, &mut CursorMemory::default(), policy(true));
    assert_eq!(split_at_sync_end(&output).1, "\x1b[2;3H\x1b[?25h");
}

#[test]
fn encoder_skips_cursor_anchor_after_sync_when_disabled() {
    let frame = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 2, 1, true, 0);
    let output = blit_with(&frame, None, &mut CursorMemory::default(), policy(false));
    assert_eq!(split_at_sync_end(&output).1, "");
}

#[test]
fn encoder_drawn_cursor_reverses_visible_cell() {
    let frame = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 2, 1, true, 6);
    let drawn = frame_with_drawn_cursor(frame.clone());

    assert_eq!(drawn.cells[5].modifier, Modifier::REVERSED.bits());
    assert_eq!(frame.cells[5].modifier, 0);

    let encoded = BlitEncoder::new().encode_with_suppressed_visible_cursor(&drawn, false);
    let output = String::from_utf8(encoded.bytes).unwrap();
    assert!(output.contains("\x1b[2;3H\x1b[6 q\x1b[?25l"));
    assert!(!output.contains("\x1b[?25h"));
    assert!(output.contains("\x1b[0;7;39;49mA"));
}

#[test]
fn encoder_drawn_cursor_leaves_hidden_cursor_cell_plain() {
    let frame = with_cursor(make_frame(1, 1, vec![plain("A")]), 0, 0, false, 0);
    assert_eq!(frame_with_drawn_cursor(frame.clone()), frame);
}

#[test]
fn encoder_drawn_cursor_clamps_to_the_last_cell() {
    let frame = with_cursor(make_frame(2, 2, vec![plain("A"); 4]), 9, 9, true, 0);
    let drawn = frame_with_drawn_cursor(frame);
    let reversed: Vec<_> = drawn.cells.iter().map(|cell| cell.modifier).collect();
    assert_eq!(reversed, [0, 0, 0, Modifier::REVERSED.bits()]);
}

#[test]
fn encoder_emits_cursor_shape_before_visibility_only_inside_sync() {
    let frame = with_cursor(make_frame(1, 1, vec![plain("A")]), 0, 0, true, 6);
    let output = blit_with(&frame, None, &mut CursorMemory::default(), policy(true));
    let final_cursor = output.find("\x1b[1;1H\x1b[6 q\x1b[?25h").unwrap();
    assert!(final_cursor < output.find(SYNC_OUTPUT_END).unwrap());
    assert_eq!(split_at_sync_end(&output).1, "\x1b[1;1H\x1b[?25h");
}

#[test]
fn encoder_repeats_hidden_cursor_anchor_after_sync() {
    let visible = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 0, 0, true, 0);
    let hidden = with_cursor(make_frame(3, 3, vec![plain("B"); 9]), 2, 1, false, 0);
    let mut memory = CursorMemory::default();
    blit_with(&visible, None, &mut memory, policy(true));
    let output = blit_with(&hidden, Some(&visible), &mut memory, policy(true));
    assert_eq!(split_at_sync_end(&output).1, "\x1b[2;3H\x1b[?25l");
}

#[test]
fn encoder_emits_osc8_link_for_linked_cells() {
    let mut frame = make_frame(3, 1, vec![linked("L", 0), linked("i", 0), plain("!")]);
    frame.hyperlinks.push("https://example.com".to_owned());
    assert_eq!(
        split_at_sync_end(&blit(&frame, None)).0,
        format!(
            "{PROLOGUE}\x1b[2J\x1b[1;1H\x1b[0;39;49m\x1b]8;;https://example.com\x1b\\Li\
             \x1b]8;;\x1b\\!\x1b[0m\x1b[1;3H\x1b[?25l"
        )
    );
}

#[test]
fn encoder_strips_control_bytes_from_links() {
    let mut frame = make_frame(1, 1, vec![linked("L", 0)]);
    frame
        .hyperlinks
        .push("https://exa\x1b\x07mple.com".to_owned());
    assert!(blit(&frame, None).contains("\x1b]8;;https://example.com\x1b\\L"));
}

#[test]
fn encoder_ascii_scroll_batches_changed_cells_per_row() {
    const WIDTH: u16 = 140;
    const HEIGHT: u16 = 50;
    let cells = usize::from(WIDTH) * usize::from(HEIGHT);
    let previous = make_frame(WIDTH, HEIGHT, vec![plain("A"); cells]);
    let current = make_frame(WIDTH, HEIGHT, vec![plain("B"); cells]);

    let output = blit(&current, Some(&previous));

    let cup_count = output.bytes().filter(|&byte| byte == b'H').count();
    assert!(
        cup_count <= usize::from(HEIGHT) + 2,
        "{cup_count} cursor moves"
    );
    assert!(output.len() <= 16_290, "{} bytes", output.len());
}

#[test]
fn encoder_batched_ascii_diff_replays_to_the_frame() {
    use maestro_app::terminal::{RenderState, Terminal};
    let previous = make_frame(4, 3, vec![plain("A"); 12]);
    let current = make_frame(4, 3, vec![plain("B"); 12]);
    let mut terminal = Terminal::new(4, 3, 0).unwrap();

    terminal.write(blit(&previous, None).as_bytes());
    terminal.write(blit(&current, Some(&previous)).as_bytes());

    let mut state = RenderState::default();
    state.update(&mut terminal);
    let screen: Vec<String> = state
        .rows()
        .map(|row| row.cells().map(|cell| cell.graphemes_utf8()).collect())
        .collect();
    assert_eq!(screen, ["BBBB", "BBBB", "BBBB"]);
}

#[test]
fn encoder_size_change_repaints_without_clearing() {
    let previous = make_frame(2, 2, vec![plain("A"); 4]);
    let current = make_frame(3, 2, vec![plain("B"); 6]);
    let mut encoder = BlitEncoder::new();
    let initial = encoder.encode(&previous, false);
    encoder.commit(previous, &initial);

    let blit = encoder.encode(&current, false);
    assert!(blit.full);
    let output = String::from_utf8(blit.bytes).unwrap();
    assert!(!output.contains("\x1b[2J"));
    assert!(output.bytes().filter(|byte| *byte == b'B').count() >= 6);
}

#[test]
fn encoder_forced_repaint_writes_all_cells_without_clearing() {
    let frame = make_frame(3, 2, vec![plain("A"); 6]);
    let mut encoder = BlitEncoder::new();
    let initial = encoder.encode(&frame, false);
    encoder.commit(frame.clone(), &initial);

    let blit = encoder.encode(&frame, true);
    assert!(blit.full);
    let output = String::from_utf8(blit.bytes).unwrap();
    assert!(!output.contains("\x1b[2J"));
    assert!(output.bytes().filter(|byte| *byte == b'A').count() >= 6);
}

#[test]
fn encoder_commit_tracks_the_shown_frame_and_cursor() {
    let first = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 1, 1, true, 5);
    let mut encoder = BlitEncoder::new();
    assert!(encoder.last_frame().is_none());
    let blit = encoder.encode(&first, false);
    assert!(blit.full);
    encoder.commit(first.clone(), &blit);
    assert!(encoder.is_current(&first));
    assert_eq!(encoder.last_frame(), Some(&first));

    let mut hidden = first;
    hidden.cursor = None;
    assert!(!encoder.is_current(&hidden));
    let blit = encoder.encode(&hidden, false);
    assert!(!blit.full);
    let output = String::from_utf8(blit.bytes).unwrap();
    assert!(output.contains("\x1b[2;2H\x1b[0 q\x1b[?25l"));
}

#[test]
fn encoder_positions_cursor() {
    let frame = with_cursor(make_frame(1, 1, vec![plain("A")]), 0, 0, true, 0);
    assert!(
        split_at_sync_end(&blit(&frame, None))
            .0
            .ends_with("A\x1b[0m\x1b[1;1H\x1b[?25h")
    );
}

#[test]
fn encoder_hides_invisible_cursor() {
    let frame = with_cursor(make_frame(1, 1, vec![plain("A")]), 0, 0, false, 0);
    assert!(
        split_at_sync_end(&blit(&frame, None))
            .0
            .ends_with("A\x1b[0m\x1b[1;1H\x1b[?25l")
    );
}

#[test]
fn encoder_hides_cursor_without_cursor_record() {
    let output = blit(&make_frame(1, 1, vec![plain("A")]), None);
    assert!(
        split_at_sync_end(&output)
            .0
            .ends_with("A\x1b[0m\x1b[1;1H\x1b[?25l")
    );
}

#[test]
fn encoder_restores_cursor_visibility() {
    let previous = with_cursor(make_frame(1, 1, vec![plain("A")]), 0, 0, false, 0);
    assert!(
        split_at_sync_end(&blit(&previous, None))
            .0
            .ends_with("A\x1b[0m\x1b[1;1H\x1b[?25l")
    );

    let current = with_cursor(make_frame(1, 1, vec![plain("B")]), 0, 0, true, 0);
    let output = blit(&current, Some(&previous));
    assert!(
        split_at_sync_end(&output)
            .0
            .ends_with("B\x1b[0m\x1b[1;1H\x1b[?25h")
    );
}

#[test]
fn encoder_positions_cursor_before_showing_it() {
    let previous = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 0, 0, true, 0);
    let mut current = with_cursor(previous.clone(), 2, 2, true, 0);
    current.cells[0] = plain("B");
    let output = blit(&current, Some(&previous));
    assert!(
        split_at_sync_end(&output)
            .0
            .ends_with("B\x1b[0m\x1b[3;3H\x1b[?25h")
    );
}

#[test]
fn encoder_parks_hidden_cursor_at_last_visible_position() {
    let visible = with_cursor(make_frame(3, 3, vec![plain("A"); 9]), 1, 1, true, 0);
    let hidden = make_frame(3, 3, vec![plain("B"); 9]);
    let mut memory = CursorMemory::default();
    blit_with(&visible, None, &mut memory, policy(!cfg!(windows)));
    let output = blit_with(&hidden, Some(&visible), &mut memory, policy(!cfg!(windows)));
    assert!(
        split_at_sync_end(&output)
            .0
            .ends_with("B\x1b[0m\x1b[2;2H\x1b[?25l")
    );
}

#[test]
fn encoder_parks_hidden_cursor_at_bottom_right_without_history() {
    let frame = make_frame(3, 2, vec![plain("A"); 6]);
    assert!(
        split_at_sync_end(&blit(&frame, None))
            .0
            .ends_with("A\x1b[0m\x1b[2;3H\x1b[?25l")
    );
}

#[test]
fn encoder_hides_previous_visible_cursor_when_next_frame_has_none() {
    let previous = with_cursor(make_frame(1, 1, vec![plain("A")]), 0, 0, true, 0);
    let current = make_frame(1, 1, vec![plain("B")]);
    let output = blit(&current, Some(&previous));
    assert!(
        split_at_sync_end(&output)
            .0
            .ends_with("B\x1b[0m\x1b[1;1H\x1b[?25l")
    );
}

#[test]
fn encoder_full_redraw_skips_wide_grapheme_tails() {
    let frame = make_frame(3, 1, vec![plain(WIDE_GRAPHEME), plain(" "), plain("Z")]);
    let output = blit(&frame, None);
    assert!(output.contains("\x1b[1;1H"));
    assert!(!output.contains("\x1b[1;2H"));
    assert!(output.contains("\x1b[1;3HZ"));
}

#[test]
fn encoder_full_redraw_skips_voiced_kana_tails() {
    let frame = make_frame(
        3,
        1,
        vec![plain(HALFWIDTH_VOICED_KANA), skip(" "), plain("Z")],
    );
    let output = blit(&frame, None);
    assert!(output.contains("\x1b[1;1H"));
    assert!(!output.contains("\x1b[1;2H"));
    assert!(output.contains("\x1b[1;3HZ"));
}

#[test]
fn encoder_diff_reveals_cells_hidden_by_wide_graphemes() {
    let previous = make_frame(3, 1, vec![plain(WIDE_GRAPHEME), plain(" "), plain("Z")]);
    let current = make_frame(3, 1, vec![plain("A"), plain(" "), plain("Z")]);
    let output = blit(&current, Some(&previous));
    assert!(output.contains("\x1b[1;1H"));
    assert!(output.contains("\x1b[1;2H"));
}

#[test]
fn encoder_diff_skips_new_wide_grapheme_tails() {
    let previous = make_frame(3, 1, vec![plain("A"), plain("B"), plain("Z")]);
    let current = make_frame(3, 1, vec![plain(WIDE_GRAPHEME), plain(" "), plain("Z")]);
    let output = blit(&current, Some(&previous));
    assert!(output.contains("\x1b[1;1H"));
    assert!(!output.contains("\x1b[1;2H"));
}

#[test]
fn encoder_diff_reveals_cells_hidden_by_voiced_kana() {
    let previous = make_frame(
        3,
        1,
        vec![plain(HALFWIDTH_VOICED_KANA), skip(" "), plain("Z")],
    );
    let current = make_frame(3, 1, vec![plain("A"), plain(" "), plain("Z")]);
    let output = blit(&current, Some(&previous));
    assert!(output.contains("\x1b[1;1H"));
    assert!(output.contains("\x1b[1;2H"));
}

#[test]
fn encoder_final_sync_output_end_finds_the_last_end() {
    assert_eq!(final_sync_output_end(b"abc"), None);
    let bytes = b"\x1b[?2026lx\x1b[?2026l\x1b[1;1H";
    assert_eq!(final_sync_output_end(bytes), Some(9));
}

#[test]
fn encoder_profile_counts_changed_cells_and_runs() {
    let previous = make_frame(4, 2, vec![plain("A"); 8]);
    let mut current = previous.clone();
    for index in [0, 1, 3, 4] {
        current.cells[index] = plain("B");
    }
    let stats = ProfBlitStats::new(&current, Some(&previous));
    let counts = (stats.scanned_cells, stats.changed_cells, stats.changed_runs);
    assert_eq!(counts, (8, 4, 3));

    let mut full = previous;
    full.cells[2] = skip("A");
    let stats = ProfBlitStats::new(&full, None);
    let counts = (stats.scanned_cells, stats.changed_cells, stats.changed_runs);
    assert_eq!(counts, (8, 7, 7));
}

/// One recorded reference encoding.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncoderCase {
    case: String,
    frame: FrameData,
    previous: Option<FrameData>,
    repeat_ime_anchor: bool,
    clear_before_full_redraw: bool,
    suppress_visible_cursor: bool,
    bytes: String,
}

#[test]
fn encoder_matches_reference_bytes_for_recorded_cases() {
    let cases: Vec<EncoderCase> =
        serde_json::from_str(include_str!("../../../tests/fixtures/encoder-corpus.json")).unwrap();
    assert_eq!(cases.len(), 19);
    for case in cases {
        let policy = BlitPolicy {
            repeat_ime_anchor: case.repeat_ime_anchor,
            clear_before_full_redraw: case.clear_before_full_redraw,
            suppress_visible_cursor: case.suppress_visible_cursor,
        };
        let bytes = blit_frame(
            &case.frame,
            case.previous.as_ref(),
            &mut CursorMemory::default(),
            policy,
        );
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            case.bytes,
            "{}",
            case.case
        );
    }
}
