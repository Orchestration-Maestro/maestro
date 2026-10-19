//! Shared host input recognition boundaries.

use maestro_tui::stdin_buffer::scan_sgr_mouse_fields;

#[test]
fn sgr_field_scanner_preserves_each_consumer_domain() {
    use crate::input::mouse::parse_report;

    for (data, geometry) in [
        (b"\x1b[<0;1;2M".as_slice(), Some((1, 2))),
        (b"\x1b[<0;1;2m", Some((1, 2))),
        (b"\x1b[<0;0;0M", Some((0, 0))),
        (b"\x1b[<256;65536;4294967295M", Some((65536, u32::MAX))),
        (b"\x1b[<0;65535;65535M", Some((65535, 65535))),
        (b"\x1b[<0;1;2;9M", None),
        (b"\x1b[<0;1;2;M", None),
        (b"\x1b[<+0;+1;+2M", None),
        (b"\x1b[<0;;2M", None),
        (b"\x1b[<0;4294967296;2M", None),
        (b"\x1b[<255;1;2M", Some((1, 2))),
    ] {
        assert!(scan_sgr_mouse_fields(data).is_some());
        assert_eq!(parse_report(data), geometry);
    }
    let fields = scan_sgr_mouse_fields(b"\x1b[<+0;;4294967296;9;10m").unwrap();
    assert_eq!(fields.cb, b"+0");
    assert_eq!(fields.column, b"");
    assert_eq!(fields.row, b"4294967296");
    assert_eq!(fields.extra, Some(b"9;10".as_slice()));
    assert!(fields.release);
    let fields = scan_sgr_mouse_fields(b"\x1b[<0;1;2M").unwrap();
    assert_eq!(fields.extra, None);
    assert!(!fields.release);
    for malformed in [
        b"\x1b[<0;1M".as_slice(),
        b"\x1b[<0;1;2X",
        b"\x1b[<0;1;2Mkey",
    ] {
        assert!(scan_sgr_mouse_fields(malformed).is_none());
    }
}
