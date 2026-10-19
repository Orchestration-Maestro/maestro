use super::super::*;
use super::{page, pin, screen, write_row};
use crate::sgr::Attribute;

#[test]
fn screen_style_basics() {
    let mut screen = Screen::init(Options::default());
    let node = screen
        .pages
        .tracked_pin(screen.cursor.page_pin)
        .unwrap()
        .node;
    assert_eq!(screen.pages.page(node).unwrap().styles.count(), 0);
    screen.set_attribute(Attribute::Bold).unwrap();
    assert_ne!(screen.cursor.style_id, 0);
    assert_eq!(screen.pages.page(node).unwrap().styles.count(), 1);
    assert!(screen.cursor.style.flags.bold());
    screen.set_attribute(Attribute::Italic).unwrap();
    assert_ne!(screen.cursor.style_id, 0);
    assert_eq!(screen.pages.page(node).unwrap().styles.count(), 1);
    assert!(screen.cursor.style.flags.italic());
}

#[test]
fn screen_style_reset_to_default() {
    let mut s = super::screen(80, 24);
    s.set_attribute(Attribute::Bold).unwrap();
    assert_eq!(super::page(&s).styles.count(), 1);
    assert_ne!(s.cursor.style_id, 0);
    s.set_attribute(Attribute::ResetBold).unwrap();
    assert_eq!(s.cursor.style_id, 0);
    assert_eq!(super::page(&s).styles.count(), 0);
}

#[test]
fn screen_style_reset_with_unset() {
    let mut s = super::screen(80, 24);
    s.set_attribute(Attribute::Bold).unwrap();
    assert_eq!(super::page(&s).styles.count(), 1);
    assert_ne!(s.cursor.style_id, 0);
    s.set_attribute(Attribute::Unset).unwrap();
    assert_eq!(s.cursor.style_id, 0);
    assert_eq!(super::page(&s).styles.count(), 0);
}

#[test]
fn screen_reasserted_attribute_keeps_style_ownership() {
    let mut s = super::screen(5, 5);
    s.set_attribute(Attribute::Bold).unwrap();
    super::write_row(&mut s, "ABCDE");
    let id = s.cursor.style_id;
    let node = super::pin(&s).node;
    s.set_attribute(Attribute::Bold).unwrap();
    s.set_attribute(Attribute::Unknown(crate::sgr::Unknown {
        full: &[999],
        partial: &[999],
    }))
    .unwrap();
    assert_eq!(s.cursor.style_id, id);
    assert_eq!(super::pin(&s).node, node);
    assert_eq!(super::page(&s).styles.ref_count(id), 6);
    s.set_attribute(Attribute::Italic).unwrap();
    assert_ne!(s.cursor.style_id, id);
    assert_eq!(super::page(&s).styles.ref_count(id), 5);
    assert_eq!(super::page(&s).styles.ref_count(s.cursor.style_id), 1);
}

#[test]
fn screen_each_attribute_changes_only_its_owned_style_fields() {
    check_flag_attributes();
    check_color_attributes();
}

fn check_flag_attributes() {
    use crate::{sgr::Underline, style::Flags};
    let mut flags = Flags::default();
    flags.set_bold(true);
    flags.set_faint(true);
    flags.set_italic(true);
    flags.set_blink(true);
    flags.set_inverse(true);
    flags.set_invisible(true);
    flags.set_strikethrough(true);
    flags.set_overline(true);
    flags.set_underline(Underline::Curly);
    let active = Style {
        flags,
        fg_color: crate::style::Color::Palette(2),
        bg_color: crate::style::Color::Palette(3),
        underline_color: crate::style::Color::Palette(4),
    };
    let resets = [
        Attribute::ResetBold,
        Attribute::ResetItalic,
        Attribute::ResetBlink,
        Attribute::ResetInverse,
        Attribute::ResetInvisible,
        Attribute::ResetStrikethrough,
        Attribute::ResetOverline,
    ];
    let masks = [3_u8, 4, 8, 16, 32, 64, 128];
    for (attribute, removed) in resets.into_iter().zip(masks) {
        let mut s = super::screen(5, 5);
        s.cursor.style = active;
        s.manual_style_update().unwrap();
        s.set_attribute(attribute).unwrap();
        assert_eq!(flag_bits(s.cursor.style.flags), 255 ^ removed);
        assert_colors(s.cursor.style, active);
        assert_eq!(s.cursor.style.flags.underline(), Underline::Curly);
    }
    let sets = [
        Attribute::Bold,
        Attribute::Faint,
        Attribute::Italic,
        Attribute::Blink,
        Attribute::Inverse,
        Attribute::Invisible,
        Attribute::Strikethrough,
        Attribute::Overline,
    ];
    for (index, attribute) in sets.into_iter().enumerate() {
        let mut s = super::screen(5, 5);
        s.cursor.style = Style {
            flags: Flags::default(),
            ..active
        };
        s.manual_style_update().unwrap();
        s.set_attribute(attribute).unwrap();
        assert_eq!(flag_bits(s.cursor.style.flags), 1 << index);
        assert_colors(s.cursor.style, active);
    }
    check_underlines(active);
}

fn check_underlines(active: Style) {
    use crate::sgr::Underline;
    for underline in [
        Underline::None,
        Underline::Single,
        Underline::Double,
        Underline::Curly,
        Underline::Dotted,
        Underline::Dashed,
    ] {
        let mut s = super::screen(5, 5);
        s.cursor.style = active;
        s.manual_style_update().unwrap();
        s.set_attribute(Attribute::Underline(underline)).unwrap();
        assert_eq!(flag_bits(s.cursor.style.flags), 255);
        assert_eq!(s.cursor.style.flags.underline(), underline);
        assert_colors(s.cursor.style, active);
    }
}

fn assert_colors(actual: Style, expected: Style) {
    assert_eq!(actual.fg_color, expected.fg_color);
    assert_eq!(actual.bg_color, expected.bg_color);
    assert_eq!(actual.underline_color, expected.underline_color);
}

fn flag_bits(flags: crate::style::Flags) -> u8 {
    [
        flags.bold(),
        flags.faint(),
        flags.italic(),
        flags.blink(),
        flags.inverse(),
        flags.invisible(),
        flags.strikethrough(),
        flags.overline(),
    ]
    .into_iter()
    .enumerate()
    .fold(0, |bits, (index, value)| bits | (u8::from(value) << index))
}

fn check_color_attributes() {
    use crate::style::Color::{None as DefaultColor, Palette, Rgb as Direct};
    use crate::{
        color::{Name, Rgb},
        style::Color,
    };
    use Attribute as A;
    let rgb = Rgb { r: 9, g: 8, b: 7 };
    let initial = Style {
        fg_color: Color::Palette(2),
        bg_color: Color::Palette(3),
        underline_color: Color::Palette(4),
        ..Style::default()
    };
    let (fg, bg, ul) = (initial.fg_color, initial.bg_color, initial.underline_color);
    let cases = [
        (A::DirectColorFg(rgb), Direct(rgb), bg, ul),
        (A::DirectColorBg(rgb), fg, Direct(rgb), ul),
        (A::Color8Fg(Name::RED), Palette(1), bg, ul),
        (A::Color8Bg(Name::BLUE), fg, Palette(4), ul),
        (A::Color8BrightFg(Name::BRIGHT_RED), Palette(9), bg, ul),
        (A::Color8BrightBg(Name::BRIGHT_BLUE), fg, Palette(12), ul),
        (A::Palette256Fg(255), Palette(255), bg, ul),
        (A::Palette256Bg(254), fg, Palette(254), ul),
        (A::ResetFg, DefaultColor, bg, ul),
        (A::ResetBg, fg, DefaultColor, ul),
        (A::UnderlineColor(rgb), fg, bg, Direct(rgb)),
        (A::Palette256UnderlineColor(253), fg, bg, Palette(253)),
        (A::ResetUnderlineColor, fg, bg, DefaultColor),
        (A::Unset, DefaultColor, DefaultColor, DefaultColor),
    ];
    for (attribute, fg_color, bg_color, underline_color) in cases {
        let mut s = super::screen(5, 5);
        s.cursor.style = initial;
        s.manual_style_update().unwrap();
        s.set_attribute(attribute).unwrap();
        assert_eq!(
            s.cursor.style,
            Style {
                fg_color,
                bg_color,
                underline_color,
                ..Style::default()
            }
        );
        if s.cursor.style_id != 0 {
            assert_eq!(
                super::page(&s).styles.get(s.cursor.style_id),
                &s.cursor.style
            );
        }
    }
}

#[test]
fn screen_increase_capacity_cursor_style_ref_count_preserved() {
    let mut s = super::screen(5, 5);
    s.set_attribute(Attribute::Bold).unwrap();
    super::write_row(&mut s, "1ABCD");
    assert_eq!(super::page(&s).styles.ref_count(s.cursor.style_id), 6);
    let old_style = s.cursor.style;
    let old = super::pin(&s);
    let new = s
        .increase_capacity(
            old.node,
            Some(crate::page_list::IncreaseCapacity::GraphemeBytes),
        )
        .unwrap();
    assert_eq!(super::pin(&s).node, new);
    assert!(!s.pages.node_is_valid(old.node));
    assert_eq!(s.cursor.style, old_style);
    assert_ne!(s.cursor.style_id, 0);
    assert_eq!(super::page(&s).styles.ref_count(s.cursor.style_id), 6);
    assert_eq!(
        super::pin(&s).row_and_cell(&s.pages).unwrap().1.codepoint(),
        'D'
    );
}

#[test]
fn screen_increase_capacity_cursor_hyperlink_ref_count_preserved() {
    let mut s = super::screen(5, 5);
    s.start_hyperlink(b"https://example.com/", None).unwrap();
    super::write_row(&mut s, "1ABCD");
    assert_eq!(super::page(&s).hyperlink_set.count(), 1);
    assert_eq!(
        super::page(&s)
            .hyperlink_set
            .ref_count(s.cursor.hyperlink_id),
        6
    );
    s.increase_capacity(
        super::pin(&s).node,
        Some(crate::page_list::IncreaseCapacity::GraphemeBytes),
    )
    .unwrap();
    assert_eq!(
        s.cursor.hyperlink.as_ref().unwrap().uri,
        b"https://example.com/"
    );
    assert_ne!(s.cursor.hyperlink_id, 0);
    assert_eq!(super::page(&s).hyperlink_set.count(), 1);
    assert_eq!(
        super::page(&s)
            .hyperlink_set
            .ref_count(s.cursor.hyperlink_id),
        6
    );
}

#[test]
fn screen_increase_capacity_cursor_with_both_style_and_hyperlink_preserved() {
    let mut s = super::screen(5, 5);
    s.set_attribute(Attribute::Bold).unwrap();
    s.start_hyperlink(b"https://example.com/", None).unwrap();
    super::write_row(&mut s, "1ABCD");
    let old = s.cursor.style;
    assert_eq!(super::page(&s).styles.ref_count(s.cursor.style_id), 6);
    assert_eq!(super::page(&s).hyperlink_set.count(), 1);
    s.increase_capacity(
        super::pin(&s).node,
        Some(crate::page_list::IncreaseCapacity::GraphemeBytes),
    )
    .unwrap();
    assert_eq!(s.cursor.style, old);
    assert_ne!(s.cursor.style_id, 0);
    assert_ne!(s.cursor.hyperlink_id, 0);
    assert_eq!(
        s.cursor.hyperlink.as_ref().unwrap().uri,
        b"https://example.com/"
    );
    assert_eq!(super::page(&s).styles.ref_count(s.cursor.style_id), 6);
    assert_eq!(super::page(&s).hyperlink_set.count(), 1);
}

#[test]
fn screen_increase_capacity_non_cursor_page_returns_early() {
    let mut s = super::screen(80, 24);
    s.set_attribute(Attribute::Bold).unwrap();
    s.start_hyperlink(b"https://example.com/", None).unwrap();
    super::write_row(&mut s, "Hello");
    let old = s.cursor.clone();
    let first = super::pin(&s).node;
    let remaining = super::page(&s).capacity.rows - super::page(&s).size.rows;
    for _ in 0..remaining {
        s.pages.grow();
    }
    let second = s.pages.grow().unwrap();
    let first_cap = s.pages.page(first).unwrap().capacity.styles;
    let second_cap = s.pages.page(second).unwrap().capacity.styles;
    let new = s
        .increase_capacity(second, Some(crate::page_list::IncreaseCapacity::Styles))
        .unwrap();
    assert_eq!(s.pages.page(new).unwrap().capacity.styles, second_cap * 2);
    assert_eq!(s.pages.page(first).unwrap().capacity.styles, first_cap);
    assert_eq!(super::pin(&s).node, first);
    assert_eq!(s.cursor.style, old.style);
    assert_eq!(s.cursor.style_id, old.style_id);
    assert_eq!(s.cursor.hyperlink, old.hyperlink);
    assert_eq!(s.cursor.hyperlink_id, old.hyperlink_id);
    assert_eq!(super::page(&s).styles.ref_count(s.cursor.style_id), 6);
}

#[test]
fn screen_set_attribute_increases_capacity_when_style_map_is_full() {
    let mut s = super::screen(10, 5);
    let old = super::pin(&s).node;
    let capacity = super::page(&s).capacity.styles;
    super::fill_styles(s.pages.page_mut(old).unwrap());
    s.set_attribute(Attribute::Bold).unwrap();
    assert!(s.cursor.style.flags.bold());
    assert_ne!(s.cursor.style_id, 0);
    assert!(super::page(&s).capacity.styles > capacity);
    assert_ne!(super::pin(&s).node, old);
}

#[test]
fn screen_set_attribute_splits_page_on_out_of_space_at_max_styles() {
    let mut s = super::screen(10, 10);
    s.cursor_absolute(4, 4);
    while super::page(&s).capacity.styles < u16::MAX {
        s.increase_capacity(
            super::pin(&s).node,
            Some(crate::page_list::IncreaseCapacity::Styles),
        )
        .unwrap();
    }
    let old = super::pin(&s).node;
    super::fill_styles(s.pages.page_mut(old).unwrap());
    s.set_attribute(Attribute::Bold).unwrap();
    assert!(s.cursor.style.flags.bold());
    assert_ne!(s.cursor.style_id, 0);
    assert_eq!(s.pages.total_pages(), 2);
    assert_ne!(super::pin(&s).node, old);
    assert_eq!((s.cursor.x, s.cursor.y), (4, 4));
    assert_eq!(
        super::page(&s).styles.get(s.cursor.style_id),
        &s.cursor.style
    );
}

#[test]
fn screen_cursor_down_across_pages_preserves_style() {
    let mut s = super::screen(10, 3);
    let (first, second) = super::split_rows(&mut s, 2);
    s.cursor_absolute(0, 1);
    s.set_attribute(Attribute::Bold).unwrap();
    assert_eq!(super::pin(&s).node, first);
    s.cursor_down(1);
    assert_eq!(super::pin(&s).node, second);
    assert!(super::page(&s).styles.get(s.cursor.style_id).flags.bold());
    assert_eq!(s.pages.page(first).unwrap().styles.count(), 0);
}

#[test]
fn screen_cursor_up_across_pages_preserves_style() {
    let mut s = super::screen(10, 3);
    let (first, second) = super::split_rows(&mut s, 2);
    s.cursor_absolute(0, 2);
    s.set_attribute(Attribute::Bold).unwrap();
    assert_eq!(super::pin(&s).node, second);
    s.cursor_up(1);
    assert_eq!(super::pin(&s).node, first);
    assert!(super::page(&s).styles.get(s.cursor.style_id).flags.bold());
    assert_eq!(s.pages.page(second).unwrap().styles.count(), 0);
}

#[test]
fn screen_cursor_absolute_across_pages_preserves_style() {
    let mut s = super::screen(10, 3);
    let (first, second) = super::split_rows(&mut s, 2);
    s.cursor_absolute(0, 2);
    s.set_attribute(Attribute::Bold).unwrap();
    s.cursor_absolute(1, 1);
    assert_eq!(super::pin(&s).node, first);
    assert_eq!((s.cursor.x, s.cursor.y), (1, 1));
    assert!(super::page(&s).styles.get(s.cursor.style_id).flags.bold());
    assert_eq!(s.pages.page(second).unwrap().styles.count(), 0);
}

#[test]
fn screen_cursor_absolute_to_page_with_insufficient_capacity() {
    let mut s = super::screen(10, 3);
    let (first, second) = super::split_rows(&mut s, 2);
    s.cursor_absolute(0, 2);
    super::fill_styles(s.pages.page_mut(first).unwrap());
    s.set_attribute(Attribute::Bold).unwrap();
    s.cursor_absolute(1, 1);
    assert_ne!(super::pin(&s).node, first);
    assert_ne!(super::pin(&s).node, second);
    assert_eq!((s.cursor.x, s.cursor.y), (1, 1));
    assert_eq!(
        super::pin(&s).row_and_cell(&s.pages).unwrap().1.style_id(),
        0
    );
    assert!(super::page(&s).styles.get(s.cursor.style_id).flags.bold());
}

#[test]
fn screen_cursor_down_to_page_with_insufficient_capacity() {
    let mut s = super::screen(10, 3);
    let (first, second) = super::split_rows(&mut s, 2);
    super::fill_styles(s.pages.page_mut(second).unwrap());
    s.cursor_absolute(0, 1);
    s.set_attribute(Attribute::Bold).unwrap();
    s.cursor_down(1);
    assert_eq!(s.cursor.y, 2);
    assert_ne!(super::pin(&s).node, first);
    assert_ne!(super::pin(&s).node, second);
    assert!(super::page(&s).styles.get(s.cursor.style_id).flags.bold());
    assert_eq!(
        super::pin(&s).row_and_cell(&s.pages).unwrap().1.style_id(),
        0
    );
}

#[test]
fn screen_cursor_copy_style_deref() {
    let source = super::screen(10, 10);
    let mut target = super::screen(10, 10);
    target.set_attribute(Attribute::Bold).unwrap();
    assert_eq!(super::page(&target).styles.count(), 1);
    assert!(target.cursor.style.flags.bold());
    target.cursor_copy(&source.cursor, true).unwrap();
    assert!(!target.cursor.style.flags.bold());
    assert_eq!(super::page(&target).styles.count(), 0);
}

#[test]
fn screen_cursor_copy_style_deref_new_page() {
    let source = super::screen(10, 10);
    let mut target = super::screen(10, 10);
    let (first, second) = super::split_rows(&mut target, 5);
    target.cursor_absolute(2, 9);
    assert_eq!(super::pin(&target).node, second);
    assert_eq!((target.cursor.x, target.cursor.y), (2, 9));
    target.set_attribute(Attribute::Bold).unwrap();
    assert_eq!(super::page(&target).styles.count(), 1);
    target.cursor_copy(&source.cursor, true).unwrap();
    assert!(!target.cursor.style.flags.bold());
    assert_eq!(target.pages.page(second).unwrap().styles.count(), 0);
    assert_eq!(super::pin(&target).node, first);
    assert_eq!((target.cursor.x, target.cursor.y), (0, 0));
}

#[test]
fn screen_cursor_copy_style_copy() {
    let mut source = super::screen(10, 10);
    source.set_attribute(Attribute::Bold).unwrap();
    let mut target = super::screen(10, 10);
    target.cursor_copy(&source.cursor, true).unwrap();
    assert!(target.cursor.style.flags.bold());
    assert_eq!(super::page(&target).styles.count(), 1);
    assert_eq!(
        super::page(&target)
            .styles
            .ref_count(target.cursor.style_id),
        1
    );
    assert_eq!(
        super::page(&source)
            .styles
            .ref_count(source.cursor.style_id),
        1
    );
}

#[test]
fn screen_scrolling_across_pages_preserves_style() {
    let mut s = screen(10, 3);
    s.set_attribute(Attribute::Bold).unwrap();
    for y in 0..3 {
        s.cursor_absolute(0, y);
        write_row(&mut s, "ABCDE");
    }
    let first = pin(&s).node;
    let remaining = page(&s).capacity.rows - page(&s).size.rows + 1;
    for _ in 0..remaining {
        s.cursor_down_scroll().unwrap();
    }
    assert_ne!(pin(&s).node, first);
    assert!(page(&s).styles.get(s.cursor.style_id).flags.bold());
    assert_eq!(page(&s).styles.ref_count(s.cursor.style_id), 1);
    assert_eq!((s.cursor.x, s.cursor.y), (4, 2));
}

#[test]
fn screen_cursor_scroll_region_up_with_styled_erased_row() {
    let mut s = screen(5, 3);
    s.no_scrollback = true;
    s.set_attribute(Attribute::Bold).unwrap();
    write_row(&mut s, "1ABCD");
    s.set_attribute(Attribute::Unset).unwrap();
    s.cursor_absolute(0, 1);
    write_row(&mut s, "2EFGH");
    s.cursor_absolute(0, 2);
    write_row(&mut s, "3IJKL");
    s.cursor_absolute(0, 2);
    s.cursor_scroll_region_up(2).unwrap();
    assert_eq!(page(&s).styles.count(), 0);
    let page = page(&s);
    for (y, expected) in ["2EFGH", "3IJKL", "     "].into_iter().enumerate() {
        let actual: String = page
            .get_cells(u16::try_from(y).unwrap())
            .iter()
            .map(|c| if c.has_text() { c.codepoint() } else { ' ' })
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn screen_storage_copy_errors_remain_typed_and_keep_partial_state() {
    for (above, reset) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut s = screen(5, 5);
        let (first, second) = super::split_rows(&mut s, 2);
        let destination = if above { second } else { first };
        let source_y = if above { 1 } else { 2 };
        s.cursor_absolute(0, source_y);
        s.set_attribute(Attribute::Bold).unwrap();
        write_row(&mut s, "STYLE");
        s.set_attribute(Attribute::Unset).unwrap();
        replace_without_style_capacity(&mut s, destination);
        s.cursor_absolute(0, if above { 0 } else { 3 });
        let before = s.pages.total_rows();
        let result = if above {
            s.cursor_scroll_above()
        } else {
            s.cursor_scroll_region_up(3)
        };
        assert_eq!(result, Err(crate::page::StorageError::StyleSetOutOfMemory));
        assert_eq!(s.pages.total_rows(), before + usize::from(above));
        let partial = s.pages.page(destination).unwrap();
        assert_eq!(
            partial.get_cells(u16::from(above))[0].codepoint(),
            if above { 'A' } else { 'B' }
        );
        assert_eq!(s.pages.page(destination).unwrap().styles.count(), 0);
        let source = if above { first } else { second };
        assert_eq!(s.pages.page(source).unwrap().styles.count(), 1);
        assert!(s.pages.pin_is_valid(super::pin(&s)));
        assert_refusal_cleanup(&mut s, [first, second], reset);
    }
}

/// Keep addressable geometry while refusing the next foreign style admission.
fn replace_without_style_capacity(s: &mut Screen, node: crate::page_list::NodeId) {
    let mut capacity = s.pages.page(node).unwrap().capacity;
    capacity.styles = 0;
    let rows = s.pages.page(node).unwrap().size.rows;
    let mut replacement = crate::Page::init(capacity);
    replacement.size.rows = rows;
    replacement.get_cells_mut(0)[0] = crate::Cell::init('A');
    replacement.get_cells_mut(1)[0] = crate::Cell::init('B');
    *s.pages.page_mut(node).unwrap() = replacement;
}

/// Release surviving cells and prove no style ownership was stranded.
fn clear_managed_page(page: &mut crate::Page) {
    for y in 0..page.size.rows {
        page.clear_cells(y, 0..page.size.cols);
    }
    assert_eq!(page.styles.count(), 0);
}

#[test]
fn screen_scroll_above_retains_managed_rows_across_pages() {
    for cursor_row in [0, 1] {
        let mut s = screen(5, 5);
        let (first, second) = super::split_rows(&mut s, 2);
        s.set_attribute(Attribute::Bold).unwrap();
        s.start_hyperlink(b"https://example.test", Some(b"stable"))
            .unwrap();
        for y in 0..5 {
            s.cursor_absolute(0, y);
            write_row(&mut s, "ABCDE");
        }
        s.cursor_absolute(0, cursor_row);
        s.cursor_scroll_above().unwrap();
        assert_eq!((s.cursor.x, s.cursor.y), (0, cursor_row));
        assert_eq!(page(&s).get_cells(super::pin(&s).y)[0].codepoint(), '\0');
        s.end_hyperlink();
        s.set_attribute(Attribute::Unset).unwrap();
        for node in [first, second] {
            let page = s.pages.page_mut(node).unwrap();
            assert_eq!(page.styles.count(), 1);
            assert_eq!(page.hyperlink_set.count(), 1);
            for y in 0..page.size.rows {
                page.clear_cells(y, 0..page.size.cols);
            }
            assert_eq!(page.styles.count(), 0);
            assert_eq!(page.hyperlink_set.count(), 0);
        }
    }
}

/// Prove a partially applied storage refusal can recover through the public reset.
fn assert_reset_usable(s: &mut Screen) {
    s.reset();
    write_row(s, "RESET");
    assert_eq!(page(s).get_cells(0)[0], crate::Cell::init('R'));
    assert_eq!((s.cursor.x, s.cursor.y), (4, 0));
}

#[test]
fn screen_region_scroll_retains_style_for_following_write() {
    let mut s = screen(5, 5);
    super::split_rows(&mut s, 3);
    for (y, text) in ["1A", "2B", "3C", "4D", "5E"].into_iter().enumerate() {
        s.cursor_absolute(0, u16::try_from(y).unwrap());
        write_row(&mut s, text);
    }
    s.cursor_absolute(0, 3);
    s.set_attribute(Attribute::Bold).unwrap();
    s.cursor_scroll_region_up(2).unwrap();
    write_row(&mut s, "X");
    assert_eq!(page(&s).get_cells(0)[0].codepoint(), 'X');
    assert_eq!(page(&s).styles.count(), 1);
    assert_eq!(page(&s).styles.ref_count(s.cursor.style_id), 2);
    assert_eq!(page(&s).get_cells(0)[0].style_id(), s.cursor.style_id);
    s.set_attribute(Attribute::Unset).unwrap();
    assert_eq!(
        page(&s)
            .styles
            .ref_count(page(&s).get_cells(0)[0].style_id()),
        1
    );
}

/// Observe last-owner release or direct reset without pre-clearing the failed state.
fn assert_refusal_cleanup(s: &mut Screen, nodes: [crate::page_list::NodeId; 2], reset: bool) {
    if reset {
        assert_reset_usable(s);
    } else {
        for node in nodes {
            clear_managed_page(s.pages.page_mut(node).unwrap());
        }
    }
}
