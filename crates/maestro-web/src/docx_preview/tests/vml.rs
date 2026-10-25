//! VML shapes rendered as SVG.
use super::check_preview;

#[test]
fn vml_shapes_render_as_svg_with_their_attributes() {
    check_preview("vml-shapes");
}

#[test]
fn vml_defaults_keep_null_and_undefined_spellings() {
    check_preview("vml-defaults");
}
