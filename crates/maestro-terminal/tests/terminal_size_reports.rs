//! Size replies and resize effects through terminal bytes.

use maestro_terminal::{
    Terminal,
    size_report::{self, Size, Style},
};

#[test]
fn size_queries_use_last_caller_geometry() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[14t\x1b[16t\x1b[18t");
    assert!(t.take_pty_reply().is_empty());
    t.resize(80, 24, 9, 18).unwrap();
    t.write(b"\x1b[14t\x1b[16t\x1b[18t");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b[4;432;720t\x1b[6;18;9t\x1b[8;24;80t"
    );
    for (width, height) in [(0, 18), (9, 0), (0, 0)] {
        t.resize(80, 24, width, height).unwrap();
        t.write(b"\x1b[14t\x1b[16t\x1b[18t");
        assert!(t.take_pty_reply().is_empty());
    }
}

#[test]
fn in_band_reports_use_clamped_geometry_on_every_successful_resize() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.resize(80, 24, 9, 18).unwrap();
    assert!(t.take_pty_reply().is_empty());
    t.write(b"\x1b[?2048h");
    t.resize(100, 40, 9, 18).unwrap();
    assert_eq!(t.take_pty_reply(), b"\x1b[48;40;100;720;900t");
    t.resize(80, 24, 9, 18).unwrap();
    t.resize(80, 24, 9, 18).unwrap();
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b[48;24;80;432;720t\x1b[48;24;80;432;720t"
    );
    for (width, height, expected) in [
        (0, 18, b"\x1b[48;24;80;432;80t".as_slice()),
        (9, 0, b"\x1b[48;24;80;24;720t".as_slice()),
        (0, 0, b"\x1b[48;24;80;24;80t".as_slice()),
    ] {
        t.resize(80, 24, width, height).unwrap();
        t.write(b"\x1b[14t\x1b[16t\x1b[18t");
        assert_eq!(t.take_pty_reply(), expected);
    }
    t.write(b"\x1b[?2048l");
    t.resize(40, 12, 9, 18).unwrap();
    assert!(t.take_pty_reply().is_empty());
}

#[test]
fn rejected_resize_preserves_queries_and_emits_no_in_band_report() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.resize(80, 24, 9, 18).unwrap();
    t.write(b"\x1b[?2048h");
    for (columns, rows) in [(0, 24), (80, 0), (0, 0)] {
        assert!(t.resize(columns, rows, 0, 0).is_err());
        t.write(b"\x1b[14t\x1b[16t\x1b[18t");
        assert_eq!(
            t.take_pty_reply(),
            b"\x1b[4;432;720t\x1b[6;18;9t\x1b[8;24;80t"
        );
    }
}

#[test]
fn size_replies_keep_full_pixel_products_when_screen_pixels_saturate() {
    let mut t = Terminal::new(2, 3, 0).unwrap();
    t.write(b"\x1b[?2048h");
    t.resize(2, 3, u32::MAX, u32::MAX).unwrap();
    assert_eq!((t.width_px, t.height_px), (u32::MAX, u32::MAX));
    t.write(b"\x1b[14t\x1b[16t\x1b[18t");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b[48;3;2;12884901885;8589934590t\x1b[4;12884901885;8589934590t\x1b[6;4294967295;4294967295t\x1b[8;3;2t"
    );
}

/// Example geometry shared by the four encoding cases.
fn example_size() -> Size {
    Size {
        rows: 24,
        columns: 80,
        cell_width: 9,
        cell_height: 18,
    }
}

#[test]
fn encode_mode_2048() {
    let mut bytes = Vec::new();
    size_report::encode(&mut bytes, Style::Mode2048, example_size()).unwrap();
    assert_eq!(bytes, b"\x1b[48;24;80;432;720t");
}

#[test]
fn encode_csi_14_t() {
    let mut bytes = Vec::new();
    size_report::encode(&mut bytes, Style::Csi14T, example_size()).unwrap();
    assert_eq!(bytes, b"\x1b[4;432;720t");
}

#[test]
fn encode_csi_16_t() {
    let mut bytes = Vec::new();
    size_report::encode(&mut bytes, Style::Csi16T, example_size()).unwrap();
    assert_eq!(bytes, b"\x1b[6;18;9t");
}

#[test]
fn encode_csi_18_t() {
    let mut bytes = Vec::new();
    size_report::encode(&mut bytes, Style::Csi18T, example_size()).unwrap();
    assert_eq!(bytes, b"\x1b[8;24;80t");
}

#[test]
fn encode_max_values_for_all_fields() {
    let size = Size {
        rows: u16::MAX,
        columns: u16::MAX,
        cell_width: u32::MAX,
        cell_height: u32::MAX,
    };
    for (style, expected) in [
        (
            Style::Mode2048,
            "\x1b[48;65535;65535;281470681677825;281470681677825t",
        ),
        (Style::Csi14T, "\x1b[4;281470681677825;281470681677825t"),
        (Style::Csi16T, "\x1b[6;4294967295;4294967295t"),
        (Style::Csi18T, "\x1b[8;65535;65535t"),
    ] {
        let mut bytes = Vec::new();
        size_report::encode(&mut bytes, style, size).unwrap();
        assert_eq!(bytes, expected.as_bytes());
    }
}
