//! Native application user-agent formatting.

/// Format the version and target with a truthful native runtime name.
#[must_use]
pub fn get_maestro_user_agent(version: &str, platform: &str, arch: &str) -> String {
    let platform = match platform {
        "windows" => "win32",
        "macos" => "darwin",
        name => name,
    };
    let arch = match arch {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        name => name,
    };
    format!("maestro/{version} ({platform}; rust; {arch})")
}
