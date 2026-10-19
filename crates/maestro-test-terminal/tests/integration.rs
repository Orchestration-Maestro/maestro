//! Terminal scenarios compiled as one integration-test program.

#[allow(
    dead_code,
    reason = "Shared input fixtures expose additional widget scenarios."
)]
#[path = "../../maestro-tui/tests/fixtures/input_support/mod.rs"]
mod input_support;
#[allow(
    dead_code,
    reason = "Support items are shared by several test scenarios."
)]
mod support;

mod cancellable_loading;
mod cell_size_input;
mod frame_styles;
mod loader_animation;
mod loader_rendering;
mod markdown_layout;
mod markdown_styles;
mod markdown_tables;
mod output_backlog;
mod overlay_focus;
mod overlay_geometry;
mod render_diagnostics;
mod render_lifecycle;
mod repeating_timers;
mod retained_frames;
mod terminal_support;
