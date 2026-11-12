//! Style attribute identity and independent packed fields.

use std::collections::HashSet;

use super::{Color, Flags, Style};
use crate::{color::Rgb, sgr::Underline};

/// One independently writable attribute.
type Attribute = (fn(Flags) -> bool, fn(&mut Flags, bool));

#[test]
fn style_fields_retain_identity() {
    let attributes: [Attribute; 8] = [
        (Flags::bold, Flags::set_bold),
        (Flags::italic, Flags::set_italic),
        (Flags::faint, Flags::set_faint),
        (Flags::blink, Flags::set_blink),
        (Flags::inverse, Flags::set_inverse),
        (Flags::invisible, Flags::set_invisible),
        (Flags::strikethrough, Flags::set_strikethrough),
        (Flags::overline, Flags::set_overline),
    ];
    let mut identities = HashSet::from([Style::default()]);
    assert!(!identities.insert(Style::default()));
    for (index, &(_, set)) in attributes.iter().enumerate() {
        let mut style = Style::default();
        set(&mut style.flags, true);
        assert!(identities.insert(style));
        for (other, &(read, _)) in attributes.iter().enumerate() {
            assert_eq!(read(style.flags), index == other);
        }
        set(&mut style.flags, false);
        assert_eq!(style, Style::default());
    }
    check_underlines(&attributes);
    let colors = [
        Color::None,
        Color::Palette(0),
        Color::Palette(7),
        Color::Rgb(Rgb::default()),
        Color::Rgb(Rgb { r: 1, g: 2, b: 3 }),
    ];
    let mut colors_seen = HashSet::new();
    for fg_color in colors {
        for bg_color in colors {
            for underline_color in colors {
                let style = Style {
                    fg_color,
                    bg_color,
                    underline_color,
                    ..Style::default()
                };
                assert!(colors_seen.insert(style));
                assert!(!colors_seen.insert(style));
            }
        }
    }
}

/// Every decoration preserves all enabled attribute bits.
fn check_underlines(attributes: &[Attribute]) {
    let mut flags = Flags::default();
    for &(_, set) in attributes {
        set(&mut flags, true);
    }
    let mut identities = HashSet::new();
    for underline in [
        Underline::None,
        Underline::Single,
        Underline::Double,
        Underline::Curly,
        Underline::Dotted,
        Underline::Dashed,
    ] {
        flags.set_underline(underline);
        assert_eq!(flags.underline(), underline);
        assert!(identities.insert(flags));
        for &(read, _) in attributes {
            assert!(read(flags));
        }
    }
}
