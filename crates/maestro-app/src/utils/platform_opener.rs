//! Native desktop viewer selection for links and authored report paths.

/// Opens `target` as one literal operand without waiting for the viewer to exit.
///
/// `platform` is the supplied operating-system observation, using
/// [`std::env::consts::OS`] names. Startup and exit failures are silent.
/// Windows uses its direct viewer rather than interpreting the target in a shell.
pub fn platform_opener(target: &str, platform: &str) {
    let program = match platform {
        "macos" => "open",
        "windows" => "explorer.exe",
        _ => "xdg-open",
    };
    let mut command = maestro_process::command(program);
    command.arg(target);
    // Opening a viewer is best-effort; its worker still reaps the child.
    drop(crate::spawn(command, None));
}
