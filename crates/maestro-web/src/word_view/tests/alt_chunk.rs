//! Alternate format chunks rendered in sandboxed frames.
use super::check_preview;

#[test]
fn alternate_chunks_render_in_sandboxed_frames() {
    check_preview("alt-chunk");
    check_preview("alt-chunk-script");
}

#[test]
fn disabled_alternate_chunks_render_nothing() {
    check_preview("alt-chunk-disabled");
}

#[test]
fn missing_alternate_chunks_leave_the_frame_empty() {
    check_preview("alt-chunk-missing-part");
}
