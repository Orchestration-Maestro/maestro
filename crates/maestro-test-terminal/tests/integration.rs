//! Terminal scenarios compiled as one integration-test program.

use maestro_tui::test_support::input_support;
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
mod markdown_quotes;
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
