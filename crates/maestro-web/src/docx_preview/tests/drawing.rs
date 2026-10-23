//! Drawing boxes, their images and their crop, rotation and anchors.
use super::check_preview;

#[test]
fn inline_images_render_inside_their_drawing_box() {
    check_preview("image-inline");
    check_preview("image-data-urls");
}

#[test]
fn anchored_drawings_float_wrap_or_collapse() {
    check_preview("image-anchors");
}

#[test]
fn cropped_and_rotated_images_scale_clip_and_rotate() {
    check_preview("image-crop-rotate");
}

#[test]
fn images_without_a_package_part_keep_no_source() {
    check_preview("image-missing-part");
}

#[test]
fn header_and_note_images_read_their_own_part_relationships() {
    check_preview("image-in-header-and-note");
}
