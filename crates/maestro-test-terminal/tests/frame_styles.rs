//! Styles never leak from one drawn line into the next.

#[allow(
    dead_code,
    reason = "Support items are shared by several test targets."
)]
mod support {
    pub mod checks;
    pub mod components;
    pub mod manual_runtime;
    pub mod recording_terminal;
    pub mod scene;
    pub mod virtual_terminal;
}
use support::scene::Scene;

#[test]
fn maestro_frames_not_leak_styles_when_a_trailing_reset_sits_beyond_the_last_visible_column_no_overlay()
 {
    let width = 20;
    let base_line = format!("\x1b[3m{}\x1b[23m", "X".repeat(width));
    let scene = Scene::new(width, 6);
    scene.probe.set_lines(&[base_line, "INPUT".to_owned()]);
    scene.tui.start().unwrap();
    scene.redraw();
    assert!(
        !scene.terminal.is_italic(1, 0),
        "the next line starts unstyled"
    );
    scene.stop();
    scene.assert_recorded("trailing_reset");
}
