use std::ffi::OsString;
use std::process::Command;

pub(super) fn run() -> Result<u8, String> {
    let output = Command::new("git")
        .args([
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--diff-filter=ACMR",
        ])
        .output()
        .map_err(|error| format!("repository tools: capture staged paths: {error}"))?;
    if !output.status.success() {
        return Ok(super::status_code(output.status));
    }
    let paths = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|bytes| !bytes.is_empty())
        .map(path_from_bytes)
        .collect::<Result<Vec<_>, _>>()?;
    let status = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["fmt", "--all"])
        .status()
        .map_err(|error| format!("repository tools: format staged: {error}"))?;
    if !status.success() {
        return Ok(super::status_code(status));
    }
    let surviving = paths
        .into_iter()
        .filter(|path| std::fs::symlink_metadata(path).is_ok())
        .collect::<Vec<_>>();
    if surviving.is_empty() {
        return Ok(0);
    }
    Command::new("git")
        .arg("add")
        .arg("--")
        .args(surviving)
        .status()
        .map(super::status_code)
        .map_err(|error| format!("repository tools: restage captured paths: {error}"))
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> Result<OsString, String> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes.to_vec()))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> Result<OsString, String> {
    String::from_utf8(bytes.to_vec())
        .map(OsString::from)
        .map_err(|error| format!("repository tools: invalid staged path: {error}"))
}
