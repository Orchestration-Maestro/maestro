//! Shared colour-conversion boundary.
use super::{RgbColor, parse_rgb_color};

#[test]
fn malformed_unicode_color_operand_is_rejected_without_panicking() {
    assert_eq!(parse_rgb_color("#éa"), None);
    let expected = Some(RgbColor {
        r: 17,
        g: 34,
        b: 51,
    });
    assert_eq!(parse_rgb_color("#123"), expected);
    assert_eq!(parse_rgb_color("rgb:1/2/3"), expected);
}
