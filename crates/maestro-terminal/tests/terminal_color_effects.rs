//! Byte-driven colour state and reply behavior.

use maestro_terminal::{Terminal, color::Rgb, device_status::ColorScheme};

#[test]
fn osc_palette_set_reset() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    let original = t.colors.palette.original[0];
    t.write(b"\x1b]4;0;rgb:ff/00/00\x1b\\");
    assert_eq!(t.colors.palette.current[0], Rgb { r: 255, g: 0, b: 0 });
    assert!(t.colors.palette.mask.is_set(0));
    t.write(b"\x1b]104;0\x1b\\");
    assert_eq!(t.colors.palette.current[0], original);
    assert!(!t.colors.palette.mask.is_set(0));
}

#[test]
fn osc_palette_reset_all() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]4;0;red;1;green;2;blue\x1b\\");
    for index in 0..3 {
        assert!(t.colors.palette.mask.is_set(index));
    }
    t.write(b"\x1b]104\x1b\\");
    assert_eq!(t.colors.palette.current, t.colors.palette.original);
    for index in 0..=255 {
        assert!(!t.colors.palette.mask.is_set(index));
    }
}

#[test]
fn osc_dynamic_set_reset() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]10;rgb:ff/00/00;rgb:00/ff/00;rgb:00/00/ff\x1b\\");
    assert_eq!(t.colors.foreground.get(), Some(Rgb { r: 255, g: 0, b: 0 }));
    assert_eq!(t.colors.background.get(), Some(Rgb { r: 0, g: 255, b: 0 }));
    assert_eq!(t.colors.cursor.get(), Some(Rgb { r: 0, g: 0, b: 255 }));
    t.write(b"\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\");
    assert_eq!(t.colors.foreground.get(), None);
    assert_eq!(t.colors.background.get(), None);
    assert_eq!(t.colors.cursor.get(), None);
}

#[test]
fn osc_color_query_responses() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]10;?;?;?\x1b\\");
    assert!(t.take_pty_reply().is_empty());
    t.write(b"\x1b]4;2;rgb:12/34/56;2;?\x1b\\");
    assert_eq!(t.take_pty_reply(), b"\x1b]4;2;rgb:1212/3434/5656\x1b\\");
    t.write(b"\x1b]10;rgb:01/02/03;rgb:04/05/06;rgb:07/08/09\x1b\\");
    t.write(b"\x1b]10;?;?;?\x1b\\");
    assert_eq!(t.take_pty_reply(), b"\x1b]10;rgb:0101/0202/0303\x1b\\\x1b]11;rgb:0404/0505/0606\x1b\\\x1b]12;rgb:0707/0808/0909\x1b\\");
    t.write(b"\x1b]112\x1b\\\x1b]12;?\x07");
    assert_eq!(t.take_pty_reply(), b"\x1b]12;rgb:0101/0202/0303\x07");
}

#[test]
fn kitty_color_palette_set_reset() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    let original = t.colors.palette.original[5];
    t.write(b"\x1b]21;5=rgb:ff/00/ff\x1b\\");
    assert_eq!(
        t.colors.palette.current[5],
        Rgb {
            r: 255,
            g: 0,
            b: 255
        }
    );
    assert!(t.colors.palette.mask.is_set(5));
    t.write(b"\x1b]21;5=\x1b\\");
    assert_eq!(t.colors.palette.current[5], original);
    assert!(!t.colors.palette.mask.is_set(5));
}

#[test]
fn kitty_color_dynamic_set_reset() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]21;foreground=rgb:12/34/56;background=rgb:78/9a/bc;cursor=rgb:de/f0/12\x1b\\");
    assert_eq!(
        t.colors.foreground.get(),
        Some(Rgb {
            r: 0x12,
            g: 0x34,
            b: 0x56
        })
    );
    assert_eq!(
        t.colors.background.get(),
        Some(Rgb {
            r: 0x78,
            g: 0x9a,
            b: 0xbc
        })
    );
    assert_eq!(
        t.colors.cursor.get(),
        Some(Rgb {
            r: 0xde,
            g: 0xf0,
            b: 0x12
        })
    );
    t.write(b"\x1b]21;foreground=;background=;cursor=\x1b\\");
    assert_eq!(t.colors.foreground.get(), None);
    assert_eq!(t.colors.background.get(), None);
    assert_eq!(t.colors.cursor.get(), None);
}

#[test]
fn kitty_color_query_responses() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]21;background=?\x1b\\");
    assert_eq!(t.take_pty_reply(), b"\x1b]21;background=\x1b\\");
    t.write(b"\x1b]21;foreground=rgb:12/34/56;2=rgb:aa/bb/cc\x1b\\");
    t.write(b"\x1b]21;foreground=?;background=?;2=?\x1b\\");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b]21;foreground=rgb:12/34/56;background=;2=rgb:aa/bb/cc\x1b\\"
    );
    t.write(b"\x1b]21;cursor=?;2=?;2=?\x07");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b]21;cursor=;2=rgb:aa/bb/cc;2=rgb:aa/bb/cc\x07"
    );
}

#[test]
fn palette_dirty_flag_on_color_change() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    for input in [
        b"\x1b]4;0;red\x1b\\".as_slice(),
        b"\x1b]104;0\x1b\\",
        b"\x1b]21;1=green\x1b\\",
        b"\x1b]21;1=\x1b\\",
    ] {
        t.flags.dirty.palette = false;
        t.write(input);
        assert!(t.flags.dirty.palette);
    }
    t.flags.dirty.palette = false;
    t.write(b"\x1b]104\x1b\\");
    assert!(!t.flags.dirty.palette);
    t.write(b"\x1b]4;255;red\x1b\\");
    t.flags.dirty.palette = false;
    t.write(b"\x1b]104\x1b\\");
    assert!(t.flags.dirty.palette);
}

#[test]
fn color_defaults_and_unsupported_targets() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    let color = Rgb { r: 1, g: 2, b: 3 };
    t.set_default_foreground_color(Some(color));
    t.set_default_background_color(Some(color));
    t.set_default_cursor_color(Some(color));
    t.write(b"\x1b]10;red;green;blue\x1b\\\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\");
    assert_eq!(t.colors.foreground.get(), Some(color));
    assert_eq!(t.colors.background.get(), Some(color));
    assert_eq!(t.colors.cursor.get(), Some(color));
    let colors = t.colors.clone();
    t.flags.dirty.palette = false;
    t.write(b"\x1b]5;0;red;0;?\x1b\\\x1b]105\x1b\\\x1b]13;red;green;blue;red;green;blue;red\x1b\\");
    t.write(b"\x1b]13;?;?;?;?;?;?;?\x1b\\\x1b]113\x1b\\\x1b]114\x1b\\\x1b]115\x1b\\\x1b]116\x1b\\\x1b]117\x1b\\\x1b]118\x1b\\\x1b]119\x1b\\");
    t.write(b"\x1b]21;selection_foreground=red;selection_background=green;cursor_text=red;visual_bell=red;second_transparent_background=red\x1b\\");
    t.write(b"\x1b]21;selection_foreground=?;selection_background=?;cursor_text=?;visual_bell=?;second_transparent_background=?\x1b\\");
    t.write(b"\x1b]21;selection_foreground=;selection_background=;cursor_text=;visual_bell=;second_transparent_background=\x1b\\");
    assert_eq!(t.colors, colors);
    assert!(!t.flags.dirty.palette);
    assert!(t.take_pty_reply().is_empty());
}

#[test]
fn color_scheme_reports_only_known_transitions_in_enabled_mode() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.set_color_scheme(Some(ColorScheme::Dark));
    t.set_color_scheme(Some(ColorScheme::Light));
    assert!(t.take_pty_reply().is_empty());
    t.write(b"\x1b[?2031h");
    assert!(t.take_pty_reply().is_empty());
    t.set_color_scheme(Some(ColorScheme::Dark));
    t.set_color_scheme(Some(ColorScheme::Dark));
    t.set_color_scheme(Some(ColorScheme::Light));
    assert_eq!(t.take_pty_reply(), b"\x1b[?997;1n\x1b[?997;2n");
    t.set_color_scheme(None);
    t.set_color_scheme(Some(ColorScheme::Dark));
    assert!(t.take_pty_reply().is_empty());
    t.write(b"\x1b[?2031l");
    t.set_color_scheme(Some(ColorScheme::Light));
    assert!(t.take_pty_reply().is_empty());
    t.write(b"\x1b[?996n");
    assert_eq!(t.take_pty_reply(), b"\x1b[?997;2n");
}

#[test]
fn osc_overrides_preserve_independent_defaults() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    let fg = Rgb {
        r: 0xaa,
        g: 0xbb,
        b: 0xcc,
    };
    let bg = Rgb {
        r: 0x11,
        g: 0x22,
        b: 0x33,
    };
    let cursor = Rgb {
        r: 0xff,
        g: 0,
        b: 0x88,
    };
    t.set_default_foreground_color(Some(fg));
    t.set_default_background_color(Some(bg));
    t.set_default_cursor_color(Some(cursor));
    t.write(b"\x1b]10;black;black\x1b\\\x1b]12;black\x1b\\");
    assert_eq!(t.effective_foreground_color(), Some(Rgb::default()));
    assert_eq!(t.colors.background.get(), Some(Rgb::default()));
    assert_eq!(t.effective_cursor_color(), Some(Rgb::default()));
    assert_eq!(t.colors.foreground.default, Some(fg));
    assert_eq!(t.colors.background.default, Some(bg));
    assert_eq!(t.colors.cursor.default, Some(cursor));
    t.write(b"\x1b]110\x1b\\\x1b]111\x1b\\\x1b]112\x1b\\");
    assert_eq!(t.effective_foreground_color(), Some(fg));
    assert_eq!(t.colors.background.get(), Some(bg));
    assert_eq!(t.effective_cursor_color(), Some(cursor));
}
