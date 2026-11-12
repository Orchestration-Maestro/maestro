//! Child entry for tests that run a daemon against real shells.
#![cfg(test)]
#![cfg(target_os = "linux")]

/// Child entry: a launch that carries a pane setup entry exits here instead of testing.
#[test]
fn setup_entry() {
    if maestro_process::terminal::setup_entry().is_some() {
        std::process::exit(1);
    }
}
