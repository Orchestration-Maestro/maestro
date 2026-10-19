//! Streaming clusters retain their scalars and cell-width effects.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, suffix};
use maestro_terminal::{
    Coordinate, Mode, Point, Terminal,
    page::{cell::Wide, layout::STD_CAPACITY},
};

#[test]
fn cluster_render_multicodepoint_grapheme_disabled_mode_2027() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, false);

    t.print('\u{1f468}').unwrap();
    t.print('\u{200d}').unwrap();
    t.print('\u{1f469}').unwrap();
    t.print('\u{200d}').unwrap();
    t.print('\u{1f467}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 6);

    for (x, cp, wide, suffix_len) in [
        (0, '\u{1f468}', Wide::Wide, Some(1)),
        (1, '\0', Wide::SpacerTail, None),
        (2, '\u{1f469}', Wide::Wide, Some(1)),
        (3, '\0', Wide::SpacerTail, None),
        (4, '\u{1f467}', Wide::Wide, None),
        (5, '\0', Wide::SpacerTail, None),
    ] {
        let value = cell(&t, x, 0);
        assert_eq!(
            (value.codepoint(), value.wide(), value.has_grapheme()),
            (cp, wide, suffix_len.is_some())
        );
        assert_eq!(suffix(&t, x, 0).map(<[char]>::len), suffix_len);
    }

    assert!(dirty(&t, 0));
}

#[test]
fn cluster_vs16_does_not_make_character_with_2027_disabled() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, false);

    t.print('\u{2764}').unwrap();
    t.print('\u{fe0f}').unwrap();

    assert_text(&t, "❤️");

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{2764}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Narrow));
    let cps = suffix(&t, 0, 0).unwrap();
    assert_eq!(cps.len(), 1);
}

#[test]
fn cluster_ignored_vs16_does_not_mark_dirty() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, false);

    t.print('\u{2764}').unwrap();
    assert!(dirty(&t, 0));

    clear_dirty(&mut t);
    t.print('\u{fe0f}').unwrap();
    assert!(!dirty(&t, 0));
}

#[test]
fn cluster_render_invalid_vs16_non_grapheme() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.print('x').unwrap();
    t.print('\u{fe0f}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), 'x');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{0}');
}

#[test]
fn cluster_invalid_vs16_does_not_mark_dirty() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, false);

    t.print('x').unwrap();
    assert!(dirty(&t, 0));

    clear_dirty(&mut t);
    t.print('\u{fe0f}').unwrap();
    assert!(!dirty(&t, 0));
}

#[test]
fn cluster_variation_selectors_apply_to_preceding_codepoint() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{1f3f4}').unwrap();
    t.print('\u{200d}').unwrap();
    t.print('\u{2620}').unwrap();
    t.print('\u{fe0f}').unwrap();

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{1f3f4}');
    assert!(value.has_grapheme());
    assert_eq!(
        &['\u{200d}', '\u{2620}', '\u{fe0f}'][..],
        suffix(&t, 0, 0).unwrap()
    );
}

#[test]
fn cluster_render_multicodepoint_grapheme_mode_2027() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{1f468}').unwrap();
    t.print('\u{200d}').unwrap();
    t.print('\u{1f469}').unwrap();
    t.print('\u{200d}').unwrap();
    t.print('\u{1f467}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    assert!(dirty(&t, 0));

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{1f468}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Wide));
    let cps = suffix(&t, 0, 0).unwrap();
    assert_eq!(cps.len(), 4);
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(
        (value.has_grapheme(), value.wide()),
        (false, Wide::SpacerTail)
    );
}

#[test]
fn cluster_keypad_sequence_vs15() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{23}').unwrap();
    t.print('\u{fe0e}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    assert!(dirty(&t, 0));

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{23}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Narrow));
}

#[test]
fn cluster_keypad_sequence_vs16() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{23}').unwrap();
    t.print('\u{fe0f}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    assert!(dirty(&t, 0));

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{23}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Wide));
}

#[test]
fn cluster_fitzpatrick_skin_tone_next_valid_base() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{1f44b}').unwrap();
    t.print('\u{1f3ff}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    assert!(dirty(&t, 0));

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{1f44b}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Wide));
}

#[test]
fn cluster_fitzpatrick_skin_tone_next_to_non_base() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{22}').unwrap();
    t.print('\u{1f3ff}').unwrap();
    t.print('\u{22}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 4);

    assert!(dirty(&t, 0));

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{22}');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{1f3ff}');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Wide));
    let value = cell(&t, 3, 0);

    assert_eq!(value.codepoint(), '\u{22}');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
}

#[test]
fn cluster_multicodepoint_grapheme_marks_dirty_on_every_codepoint() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{1f468}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('\u{200d}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('\u{1f469}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('\u{200d}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('\u{1f467}').unwrap();
    assert!(dirty(&t, 0));

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);
}

#[test]
fn cluster_vs15_to_make_narrow_character() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{2614}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    t.print('\u{fe0e}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    assert_text(&t, "☔︎");

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{2614}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Narrow));
    let cps = suffix(&t, 0, 0).unwrap();
    assert_eq!(cps.len(), 1);
}

#[test]
fn cluster_vs15_on_already_narrow_emoji() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{26c8}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('\u{fe0e}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    assert_text(&t, "⛈︎");

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{26c8}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Narrow));
    let cps = suffix(&t, 0, 0).unwrap();
    assert_eq!(cps.len(), 1);
}

#[test]
fn cluster_render_invalid_vs15_following_emoji_is_wide() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{1F9E0}').unwrap();
    t.print('\u{fe0e}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{1F9E0}');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Wide));
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(value.wide(), Wide::SpacerTail);
}

#[test]
fn cluster_render_invalid_vs15_in_emoji_zwj_sequence() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{1F469}').unwrap();
    t.print('\u{fe0e}').unwrap();
    t.print('\u{200D}').unwrap();
    t.print('\u{1F466}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{1F469}');
    assert!(value.has_grapheme());
    assert_eq!(&['\u{200D}', '\u{1F466}'][..], suffix(&t, 0, 0).unwrap());
    assert_eq!(value.wide(), Wide::Wide);
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(value.wide(), Wide::SpacerTail);
}

#[test]
fn cluster_vs16_to_make_wide_character_on_next_line() {
    let mut t = Terminal::new(3, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.cursor_right(2);
    t.print('#').unwrap();
    assert_eq!(t.screens.active().cursor.x, 2);
    assert!(t.screens.active().cursor.pending_wrap);
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);

    t.print('\u{fe0f}').unwrap();

    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    assert_eq!(t.screens.active().cursor.y, 1);
    assert_eq!(t.screens.active().cursor.x, 2);
    assert!(!t.screens.active().cursor.pending_wrap);

    let value = cell(&t, 2, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(
        (value.has_grapheme(), value.wide()),
        (false, Wide::SpacerHead)
    );
    let value = cell(&t, 0, 1);

    assert_eq!(value.codepoint(), '#');
    assert_eq!(
        (value.has_grapheme(), suffix(&t, 0, 1)),
        (true, Some(&['\u{fe0f}'][..]))
    );
    assert_eq!(value.wide(), Wide::Wide);
    let value = cell(&t, 1, 1);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(
        (value.has_grapheme(), value.wide()),
        (false, Wide::SpacerTail)
    );
}

#[test]
fn cluster_vs16_to_make_wide_character_with_pending_wrap() {
    let mut t = Terminal::new(3, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.cursor_right(1);
    t.print('#').unwrap();
    assert_eq!(t.screens.active().cursor.x, 2);
    assert!(!t.screens.active().cursor.pending_wrap);

    t.print('\u{fe0f}').unwrap();

    assert_eq!(t.screens.active().cursor.x, 2);
    assert_eq!(t.screens.active().cursor.y, 0);
    assert!(t.screens.active().cursor.pending_wrap);

    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '#');
    assert!(value.has_grapheme());
    assert_eq!(&['\u{fe0f}'][..], suffix(&t, 1, 0).unwrap());
    assert_eq!(value.wide(), Wide::Wide);
    let value = cell(&t, 2, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(
        (value.has_grapheme(), value.wide()),
        (false, Wide::SpacerTail)
    );
}

#[test]
fn cluster_vs16_to_make_wide_character_with_mode_2027() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{2764}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('\u{fe0f}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);

    assert_text(&t, "❤️");

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{2764}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Wide));
    let cps = suffix(&t, 0, 0).unwrap();
    assert_eq!(cps.len(), 1);
}

#[test]
fn cluster_vs16_repeated_with_mode_2027() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{2764}').unwrap();
    t.print('\u{fe0f}').unwrap();
    t.print('\u{2764}').unwrap();
    t.print('\u{fe0f}').unwrap();

    assert!(dirty(&t, 0));

    assert_text(&t, "❤️❤️");

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{2764}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Wide));
    let cps = suffix(&t, 0, 0).unwrap();
    assert_eq!(cps.len(), 1);
    let value = cell(&t, 2, 0);

    assert_eq!(value.codepoint(), '\u{2764}');
    assert_eq!((value.has_grapheme(), value.wide()), (true, Wide::Wide));
    let cps = suffix(&t, 2, 0).unwrap();
    assert_eq!(cps.len(), 1);
}

#[test]
fn cluster_render_invalid_vs16_grapheme() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('x').unwrap();
    t.print('\u{fe0f}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), 'x');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(value.wide(), Wide::Narrow);
}

#[test]
fn cluster_render_invalid_vs16_with_second_char() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('x').unwrap();
    t.print('\u{fe0f}').unwrap();
    t.print('y').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), 'x');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));

    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), 'y');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
}

#[test]
fn cluster_render_grapheme_o_o_with_nonspacing_mark_should_be_narrow() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('o').unwrap();
    t.print('\u{300}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), 'o');
    assert!(value.has_grapheme());
    assert_eq!(&['\u{300}'][..], suffix(&t, 0, 0).unwrap());
    assert_eq!(value.wide(), Wide::Narrow);
}

#[test]
fn cluster_render_devanagari_grapheme_should_be_wide() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{915}').unwrap();
    t.print('\u{94d}').unwrap();
    t.print('\u{200d}').unwrap();
    t.print('\u{937}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), '\u{915}');
    assert!(value.has_grapheme());
    assert_eq!(
        &['\u{94d}', '\u{200d}', '\u{937}'][..],
        suffix(&t, 0, 0).unwrap()
    );
    assert_eq!(value.wide(), Wide::Wide);
    let value = cell(&t, 1, 0);

    assert_eq!(value.wide(), Wide::SpacerTail);
}

#[test]
fn cluster_render_devanagari_grapheme_should_be_wide_on_next_line() {
    let mut t = Terminal::new(3, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.cursor_right(2);

    t.print('\u{915}').unwrap();
    t.print('\u{94d}').unwrap();
    t.print('\u{200d}').unwrap();
    assert_eq!(t.screens.active().cursor.x, 2);
    assert!(t.screens.active().cursor.pending_wrap);

    t.print('\u{937}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 1);
    assert_eq!(t.screens.active().cursor.x, 2);
    assert!(!t.screens.active().cursor.pending_wrap);

    let value = cell(&t, 2, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(
        (value.has_grapheme(), value.wide()),
        (false, Wide::SpacerHead)
    );
    let value = cell(&t, 0, 1);

    assert_eq!(value.codepoint(), '\u{915}');
    assert!(value.has_grapheme());
    assert_eq!(
        &['\u{94d}', '\u{200d}', '\u{937}'][..],
        suffix(&t, 0, 1).unwrap()
    );
    assert_eq!(value.wide(), Wide::Wide);
    let value = cell(&t, 1, 1);

    assert_eq!(value.wide(), Wide::SpacerTail);
}

#[test]
fn cluster_render_invalid_vs16_with_second_char_combining() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('n').unwrap();
    t.print('\u{fe0f}').unwrap();
    t.print('\u{303}').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), 'n');
    assert!(value.has_grapheme());
    assert_eq!(&['\u{0303}'][..], suffix(&t, 0, 0).unwrap());
    assert_eq!(value.wide(), Wide::Narrow);
    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{0}');
    assert_eq!(value.wide(), Wide::Narrow);
}

#[test]
fn cluster_overwrite_grapheme_should_clear_grapheme_data() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::GraphemeCluster, true);

    t.print('\u{26c8}').unwrap();
    t.print('\u{fe0e}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);

    t.set_cursor_pos(1, 1);
    t.print('A').unwrap();
    assert!(dirty(&t, 0));

    assert_text(&t, "A");

    let value = cell(&t, 0, 0);

    assert_eq!(value.codepoint(), 'A');
    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
}

#[test]
fn cluster_render_breaks_valid_grapheme_cluster_with_prepend_ascii_for_speed() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, true);

    t.print('_').unwrap();

    t.print('\u{600}').unwrap();
    t.print('1').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 3);

    let value = cell(&t, 1, 0);

    assert_eq!(value.codepoint(), '\u{600}');
    assert!(!value.has_grapheme());

    assert_eq!(value.wide(), Wide::Narrow);
    let value = cell(&t, 2, 0);

    assert_eq!(value.codepoint(), '1');

    assert_eq!((value.has_grapheme(), value.wide()), (false, Wide::Narrow));
}

#[test]
fn cluster_enabling_grapheme_mode_handles_stored_breaks() {
    let mut t = Terminal::new(5, 1, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, false);
    t.print_slice(&['a', '\u{200b}']).unwrap();
    t.modes.set(Mode::GraphemeCluster, true);
    t.print('\u{301}').unwrap();
    assert_text(&t, "a\u{200b}\u{301}");
}

#[test]
fn cluster_vs15_to_make_narrow_character_with_pending_wrap() {
    let mut t = Terminal::new(4, 5, 0).unwrap();
    assert!(t.modes.get(Mode::Wraparound));
    t.print_slice(&['🍋', '☔']).unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    assert_eq!(
        (
            t.screens.active().cursor.x,
            t.screens.active().cursor.y,
            t.screens.active().cursor.pending_wrap
        ),
        (3, 0, true)
    );
    t.print('\u{fe0e}').unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    assert_eq!(
        (
            t.screens.active().cursor.x,
            t.screens.active().cursor.y,
            t.screens.active().cursor.pending_wrap
        ),
        (3, 0, false)
    );
    assert_text(&t, "🍋☔︎");
    assert_eq!(
        (
            cell(&t, 2, 0).codepoint(),
            cell(&t, 2, 0).wide(),
            cell(&t, 2, 0).has_grapheme()
        ),
        ('☔', Wide::Narrow, true)
    );
    assert_eq!(suffix(&t, 2, 0).unwrap().len(), 1);
    assert_eq!(
        (cell(&t, 0, 0).codepoint(), cell(&t, 0, 0).wide()),
        ('🍋', Wide::Wide)
    );
    assert_eq!(
        (cell(&t, 1, 0).codepoint(), cell(&t, 1, 0).wide()),
        ('\0', Wide::SpacerTail)
    );
}

#[test]
fn cluster_vs16_to_make_wide_character_on_next_line_with_hyperlink() {
    let mut t = Terminal::new(3, 5, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.cursor_right(2);
    t.print('#').unwrap();
    assert_eq!(
        (
            t.screens.active().cursor.x,
            t.screens.active().cursor.pending_wrap
        ),
        (2, true)
    );
    t.print('\u{fe0f}').unwrap();
    assert_eq!(
        (
            t.screens.active().cursor.x,
            t.screens.active().cursor.y,
            t.screens.active().cursor.pending_wrap
        ),
        (2, 1, false)
    );
    for (x, y, cp, wide) in [
        (2, 0, '\0', Wide::SpacerHead),
        (0, 1, '#', Wide::Wide),
        (1, 1, '\0', Wide::SpacerTail),
    ] {
        let value = cell(&t, x, y);
        assert_eq!(
            (value.codepoint(), value.wide(), value.hyperlink()),
            (cp, wide, true)
        );
    }
    assert!(super::support::terminal::row(&t, 0).wrap());
    assert!(cell(&t, 0, 1).has_grapheme());
    assert_eq!(suffix(&t, 0, 1), Some(&['\u{fe0f}'][..]));
}

#[test]
fn cluster_render_devanagari_grapheme_should_be_wide_on_next_page() {
    let (cols, rows) = (STD_CAPACITY.cols, STD_CAPACITY.rows);
    let mut t = Terminal::new(cols, rows, 0).unwrap();
    t.cursor_down(usize::from(rows - 1));
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 0 }))
        .unwrap();
    let capacity_rows = screen.pages.page(pin.node).unwrap().capacity.rows;
    for _ in rows..capacity_rows {
        t.index().unwrap();
    }
    t.cursor_right(usize::from(cols - 1));
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (cols - 1, rows - 1)
    );
    t.print_slice(&['क', '\u{94d}', '\u{200d}']).unwrap();
    assert_eq!(
        (
            t.screens.active().cursor.x,
            t.screens.active().cursor.pending_wrap
        ),
        (cols - 1, true)
    );
    t.print('ष').unwrap();
    assert_eq!(
        (
            t.screens.active().cursor.x,
            t.screens.active().cursor.y,
            t.screens.active().cursor.pending_wrap
        ),
        (2, rows - 1, false)
    );
    let previous = cell(&t, cols - 1, u32::from(rows - 2));
    assert_eq!(
        (
            previous.codepoint(),
            previous.wide(),
            previous.has_grapheme()
        ),
        ('\0', Wide::SpacerHead, false)
    );
    let head = cell(&t, 0, u32::from(rows - 1));
    assert_eq!(
        (head.codepoint(), head.wide(), head.has_grapheme()),
        ('क', Wide::Wide, true)
    );
    assert_eq!(
        suffix(&t, 0, u32::from(rows - 1)),
        Some(&['\u{94d}', '\u{200d}', 'ष'][..])
    );
    assert_eq!(cell(&t, 1, u32::from(rows - 1)).wide(), Wide::SpacerTail);
}

fn overwrite_family(column: usize, expected: &str) {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print_string("👨‍👩‍👧").unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (2, 0)
    );
    let pin = *t
        .screens
        .active()
        .pages
        .tracked_pin(t.screens.active().cursor.page_pin)
        .unwrap();
    assert_eq!(
        t.screens
            .active()
            .pages
            .page(pin.node)
            .unwrap()
            .grapheme_count(),
        1
    );
    t.set_cursor_pos(1, column);
    clear_dirty(&mut t);
    t.print('X').unwrap();
    assert!(dirty(&t, 0));
    assert_eq!(
        (
            usize::from(t.screens.active().cursor.x),
            t.screens.active().cursor.y
        ),
        (column, 0)
    );
    assert_eq!(
        t.screens
            .active()
            .pages
            .page(pin.node)
            .unwrap()
            .grapheme_count(),
        0
    );
    assert_text(&t, expected);
}

#[test]
fn cluster_overwrite_multicodepoint_grapheme_clears_grapheme_data() {
    overwrite_family(1, "X");
}

#[test]
fn cluster_overwrite_multicodepoint_grapheme_tail_clears_grapheme_data() {
    overwrite_family(2, " X");
}

fn assert_widths(cases: &[(&str, u16)]) {
    for &(text, width) in cases {
        let mut t = Terminal::new(80, 5, 0).unwrap();
        t.print_string(text).unwrap();
        assert_eq!(
            (t.screens.active().cursor.x, t.screens.active().cursor.y),
            (width, 0),
            "{text:?}"
        );
    }
}

#[test]
fn printed_clusters_use_recorded_cell_widths() {
    assert_widths(&[
        ("❤️", 2),
        ("x\u{fe0f}\u{fe0f}", 1),
        ("⌚\u{fe0e}\u{fe0f}", 1),
        ("🏴‍☠️", 2),
        ("👨‍👩‍👧", 2),
        ("#\u{fe0f}\u{20e3}", 2),
        ("1\u{20e3}", 1),
        ("👋🏿", 2),
        ("🇦🇧🇨", 4),
        ("ab", 2),
        ("\u{301}\u{302}", 0),
    ]);
}

#[test]
fn printed_cluster_variation_selectors() {
    assert_widths(&[
        ("❤️", 2),
        ("#\u{fe0f}", 2),
        ("x\u{fe0f}", 1),
        ("x\u{fe0f}\u{fe0f}", 1),
        ("#\u{fe0e}", 1),
        ("⌚\u{fe0e}", 1),
        ("⌚\u{fe0e}\u{fe0f}", 1),
        ("🏴‍☠️", 2),
    ]);
}

#[test]
fn printed_cluster_emoji_sequences() {
    assert_widths(&[
        ("👨‍👩‍👧", 2),
        ("#\u{fe0f}\u{20e3}", 2),
        ("1\u{20e3}", 1),
        ("👋🏿", 2),
    ]);
}

#[test]
fn printed_cluster_spacing_marks_can_widen_narrow_clusters() {
    assert_eq!(maestro_terminal::unicode::codepoint_width('ः'), 1);
    let mut t = Terminal::new(80, 5, 0).unwrap();
    t.print_string("aः").unwrap();
    assert_eq!(t.screens.active().cursor.x, 2);
    assert_eq!(cell(&t, 0, 0).wide(), Wide::Wide);
    assert_eq!(suffix(&t, 0, 0), Some(&['ः'][..]));
}

#[test]
fn printed_cluster_segmentation() {
    assert_widths(&[
        ("a", 1),
        ("ab", 2),
        ("🇦🇧🇨", 4),
        ("🇨", 2),
        ("", 0),
        ("\u{301}\u{302}", 0),
    ]);
}

#[test]
fn grapheme_clusters_advance_the_cursor_by_their_width() {
    assert_widths(&[
        ("", 0),
        ("e\u{301}", 1),
        ("⚠️", 2),
        ("⚠︎", 1),
        ("🇧🇷", 2),
        ("👍🏽", 2),
        ("👨‍👩‍👧", 2),
    ]);
}
