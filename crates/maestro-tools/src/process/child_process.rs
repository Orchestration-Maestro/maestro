//! Platform policy for commands requiring Windows shell interpretation.

/// Whether the host requires a shell for this command spelling.
#[must_use]
pub fn should_use_windows_shell(command: &str) -> bool {
    windows_shell(command, cfg!(windows))
}

/// Applies the explicit platform policy to the lexical command basename.
fn windows_shell(command: &str, windows: bool) -> bool {
    if !windows {
        return false;
    }
    let name = maestro_path::win32::basename(command, None).to_lowercase();
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| matches!(extension, "cmd" | "bat"))
        || matches!(
            name.as_str(),
            "npm" | "npx" | "pnpm" | "yarn" | "yarnpkg" | "corepack"
        )
}

#[cfg(test)]
mod tests;
