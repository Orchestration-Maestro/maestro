//! Resolution of the pinned search programs that native tool tests install.

/// Resolves a pinned development tool from the supplied search PATH.
///
/// # Errors
///
/// Returns an error if the supplied PATH has no absolute executable, the executable
/// fails or reports another version, or its path or output is not UTF-8.
pub fn pinned_search_tool(
    name: &str,
    tool: &str,
    version: &str,
    search: &std::ffi::OsStr,
) -> Result<String, String> {
    let executable = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let path = std::env::split_paths(search)
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("{name}: no executable on PATH"))?;
    if !path.is_absolute() {
        return Err(format!(
            "{name}: executable path is not absolute: {}",
            path.display()
        ));
    }
    let output = std::process::Command::new(&path)
        .arg("--version")
        .output()
        .map_err(|error| format!("{} --version: {error}", path.display()))?;
    let reported =
        String::from_utf8(output.stdout).map_err(|error| format!("{name} --version: {error}"))?;
    let mut words = reported.split_whitespace();
    if !output.status.success() || words.next() != Some(tool) || words.next() != Some(version) {
        return Err(format!(
            "{name}: expected {tool} {version}, got {reported:?}"
        ));
    }
    executable_text(name, &path)
}

/// Returns the resolved executable path as text.
///
/// # Errors
///
/// Returns an error naming `name` and the path when the spelling is not UTF-8.
pub fn executable_text(name: &str, path: &std::path::Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("{name}: executable path is not UTF-8: {}", path.display()))
}
