//! Pixel measurements use the screenshot's actual raster geometry.

use super::*;

/// Transparent or differently coloured pixels cannot stand in for opaque role ink.
#[test]
fn report_counts_only_opaque_matching_pixels() {
    let pixels = [1, 2, 3, 255, 1, 2, 3, 128, 1, 2, 4, 255, 1, 2, 3, 255];
    assert_eq!(counted(&pixels, [1, 2, 3]), 2);
    assert_eq!(counted(&pixels, [1, 2, 4]), 1);
    assert_eq!(counted(&pixels, [8, 9, 0]), 0);
}

/// Multiple raster rows contribute to one horizontal ink extent.
#[test]
fn report_ink_extent_uses_actual_bitmap_rows() {
    let pixels = [
        0, 0, 0, 255, 1, 2, 3, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 1, 2, 3, 255,
    ];
    assert_eq!(ink_width(&pixels, 3, [0, 0, 0]).unwrap(), 2);
    assert_eq!(ink_width(&[0, 0, 0, 255], 1, [0, 0, 0]).unwrap(), 0);
    assert!(ink_width(&pixels, 0, [0, 0, 0]).is_err());
    assert!(ink_width(&[1, 2, 3], 1, [0, 0, 0]).is_err());
}

/// An absent numeric observation cannot be silently substituted with zero.
#[test]
fn report_requires_numeric_browser_measurements() {
    assert!((number(&json!({"ink":12}), "ink").unwrap() - 12.0).abs() < f64::EPSILON);
    for value in [json!({}), json!({"ink":null}), json!({"ink":"12"})] {
        assert!(number(&value, "ink").is_err());
    }
}
