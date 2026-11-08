//! Mouse report and coordinate boundary observations.

use super::*;

/// Parsing admits only a complete report with three ASCII decimal fields.
#[test]
fn sgr_report_requires_complete_numeric_fields() {
    for (input, expected) in [
        (b"\x1b[<35;321;241M".as_slice(), Some((321, 241))),
        (b"\x1b[<0;1;2m".as_slice(), Some((1, 2))),
        (b"key".as_slice(), None),
        (b"\x1b[<0;1;2Mkey".as_slice(), None),
        (b"\x1b[<0;1M".as_slice(), None),
        (b"\x1b[<000;000;002M".as_slice(), Some((0, 2))),
        (b"\x1b[<0;1;2;3M".as_slice(), None),
    ] {
        assert_eq!(parse_report(input), expected);
    }
    for field in 0..3 {
        for invalid in ["", "-1", "+1", "é", "4294967296", "1 "] {
            let mut fields = ["1", "2", "3"];
            fields[field] = invalid;
            assert_eq!(
                parse_report(format!("\x1b[<{}M", fields.join(";")).as_bytes()),
                None
            );
        }
        let mut fields = ["1", "2", "3"];
        fields[field] = "4294967295";
        let expected = match field {
            0 => (2, 3),
            1 => (u32::MAX, 3),
            _ => (2, u32::MAX),
        };
        assert_eq!(
            parse_report(format!("\x1b[<{}m", fields.join(";")).as_bytes()),
            Some(expected)
        );
    }
}

/// Fractional host cells retain exact first and last pane pixels.
#[test]
fn fractional_host_pixels_reach_pane_edges() {
    let geometry = HostGeometry::new(211, 57, 2537, 1429).unwrap();
    let inner = maestro_workspace::tiling::Rect::new(157, 7, 53, 49);
    for (x, y, expected) in [
        (1888, 176, Position::Pixels { x: 1, y: 1 }),
        (2524, 1403, Position::Pixels { x: 636, y: 1225 }),
    ] {
        assert_eq!(
            HostPixels { x, y, geometry }.pane_position(inner, 636, 1225),
            Some(expected)
        );
    }
}

/// One-based pixels map to zero-based cells without accepting outside pixels.
#[test]
fn host_pixels_reject_outside_and_map_cells() {
    let geometry = HostGeometry::new(80, 24, 800, 480).unwrap();
    for (x, y, expected) in [
        (1, 1, Some((0, 0))),
        (800, 480, Some((79, 23))),
        (801, 1, None),
        (0, 1, None),
        (1, 481, None),
        (1, 0, None),
        (10, 20, Some((0, 0))),
        (11, 21, Some((1, 1))),
    ] {
        assert_eq!(geometry.cell(x, y), expected);
    }
    let largest = HostGeometry::new(u16::MAX, u16::MAX, u32::MAX, u32::MAX).unwrap();
    assert_eq!(largest.cell(1, 1), Some((0, 0)));
    assert_eq!(largest.cell(u32::MAX, u32::MAX), Some((65534, 65534)));
    assert_eq!(largest.cell(65537, 65537), Some((0, 0)));
    assert_eq!(largest.cell(65538, 65538), Some((1, 1)));
}

/// Constructor admission and direct-record consumers reject zero dimensions.
#[test]
fn mouse_geometry_requires_nonzero_dimensions() {
    let good = HostGeometry::new(80, 24, 800, 480).unwrap();
    let inner = maestro_workspace::tiling::Rect::new(0, 0, 1, 1);
    assert_eq!(good.cell(1, 1), Some((0, 0)));
    for (cols, rows, width_px, height_px) in [
        (0, 24, 800, 480),
        (80, 0, 800, 480),
        (80, 24, 0, 480),
        (80, 24, 800, 0),
    ] {
        assert_eq!(HostGeometry::new(cols, rows, width_px, height_px), None);
        let geometry = HostGeometry {
            cols,
            rows,
            width_px,
            height_px,
        };
        assert_eq!(geometry.cell(1, 1), None);
        assert_eq!(
            HostPixels {
                x: 1,
                y: 1,
                geometry
            }
            .pane_position(inner, 10, 20),
            None
        );
    }
}

/// Pane mapping rejects invalid rectangles, outside pixels and empty targets.
#[test]
fn pane_pixel_mapping_rejects_invalid_rectangles() {
    use maestro_workspace::tiling::Rect;
    let geometry = HostGeometry::new(80, 24, 800, 480).unwrap();
    let point = HostPixels {
        x: 101,
        y: 41,
        geometry,
    };
    let inner = Rect::new(10, 2, 2, 2);
    assert_eq!(
        point.pane_position(inner, 37, 51),
        Some(Position::Pixels { x: 1, y: 1 })
    );
    assert_eq!(
        HostPixels {
            x: 120,
            y: 80,
            geometry
        }
        .pane_position(inner, 37, 51),
        Some(Position::Pixels { x: 36, y: 50 })
    );
    for (x, y) in [
        (100, 41),
        (121, 41),
        (101, 40),
        (101, 81),
        (0, 41),
        (101, 0),
    ] {
        assert_eq!(
            HostPixels { x, y, geometry }.pane_position(inner, 37, 51),
            None
        );
    }
    for invalid in [
        Rect::new(10, 2, 0, 2),
        Rect::new(10, 2, 2, 0),
        Rect::new(79, 2, 2, 2),
        Rect::new(10, 23, 2, 2),
        Rect {
            x: u16::MAX,
            y: 0,
            width: 1,
            height: 1,
        },
        Rect {
            x: 0,
            y: u16::MAX,
            width: 1,
            height: 1,
        },
    ] {
        assert_eq!(point.pane_position(invalid, 37, 51), None);
    }
    assert_eq!(point.pane_position(inner, 0, 51), None);
    assert_eq!(point.pane_position(inner, 37, 0), None);
    assert_fractional_scaling();
}

/// Relocation preserves button spelling and terminator without validating fields.
#[test]
fn mouse_report_relocation_preserves_button_and_terminator() {
    for (input, column, row, expected) in [
        (
            b"\x1b[<003;10;20M".as_slice(),
            0,
            65535,
            Some(b"\x1b[<003;1;65536M".as_slice()),
        ),
        (
            b"\x1b[<07;9;8m".as_slice(),
            65535,
            0,
            Some(b"\x1b[<07;65536;1m".as_slice()),
        ),
        (
            b"\x1b[<xM".as_slice(),
            0,
            1,
            Some(b"\x1b[<x;1;2M".as_slice()),
        ),
        (b"x;1;2M".as_slice(), 0, 0, None),
        (b"\x1b[<0;1;2".as_slice(), 0, 0, None),
        (b"\x1b[<\xff;1;2M".as_slice(), 0, 0, None),
    ] {
        assert_eq!(report_at_cell(input, column, row).as_deref(), expected);
    }
    assert_eq!(parse_report(b"\x1b[<xM"), None);
}

/// Fractional cell boundaries scale after selecting the pane pixel span.
fn assert_fractional_scaling() {
    use maestro_workspace::tiling::Rect;
    let geometry = HostGeometry::new(3, 2, 10, 7).unwrap();
    assert_eq!(
        HostPixels {
            x: 5,
            y: 5,
            geometry
        }
        .pane_position(Rect::new(1, 1, 2, 1), 14, 20),
        Some(Position::Pixels { x: 3, y: 6 })
    );
}
