//! Resolution of the pinned search programs that native tool tests install.

/// Exact executable identity and version probes.
#[path = "search_tools/resolution.rs"]
pub mod resolution;
use resolution::pinned_search_tool;
/// The exported variable, native program name and version identity of each search program.
pub const PROGRAMS: [(&str, &str, &str); 2] = [
    ("MAESTRO_TEST_RG", "rg", "ripgrep"),
    ("MAESTRO_TEST_FD", "fd", "fd"),
];

/// Resolve the search programs the workspace pins, without evaluating its configuration.
///
/// # Errors
/// Returns unreadable configuration, a missing search PATH or a missing or wrong-version pin.
pub fn resolve(workspace: &std::path::Path) -> Result<Vec<(&'static str, String)>, String> {
    let config = workspace.join("mise.toml");
    if !config.is_file() {
        return Ok(Vec::new());
    }
    let config = config
        .to_str()
        .ok_or("the tool configuration path is not UTF-8")?;
    // Once the whole file reads, a key lookup fails only for a key the file does not hold.
    mise_output(&["config", "get", "--file", config])?;
    let search = std::env::var_os("PATH").ok_or("PATH is not set")?;
    let mut entries = Vec::new();
    for (variable, name, tool) in PROGRAMS {
        let key = format!("tools.{tool}");
        let Ok(pin) = mise_output(&["config", "get", "--file", config, &key]) else {
            continue;
        };
        entries.push((variable, pinned_search_tool(name, tool, &pin, &search)?));
    }
    Ok(entries)
}

/// Read stored tool pins without evaluating project configuration.
fn mise_output(args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("mise")
        .env("MISE_NO_CONFIG", "1")
        .args(args)
        .output()
        .map_err(|error| format!("mise {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "mise {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("mise {}: {error}", args.join(" ")))
}
