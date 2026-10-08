use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Failure message printed when formatting or verification fails.
const CHECK_FAILURE: &str = "❌ Checks failed. Please fix the errors before committing.";

/// Format captured index paths, restage surviving files and run checks.
///
/// # Errors
/// Returns native process and filesystem failures.
pub fn run(
    root: &Path,
    format: &mut Command,
    check: &mut Command,
    smoke: Option<&mut Command>,
) -> io::Result<ExitCode> {
    let paths = capture(root)?;
    println!("Running formatting, linting, and type checking...");
    if !format
        .status()
        .inspect_err(|error| eprintln!("{error}"))
        .is_ok_and(|status| status.success())
    {
        println!("{CHECK_FAILURE}");
        return Ok(ExitCode::FAILURE);
    }
    let surviving: Vec<_> = paths
        .iter()
        .filter(|path| root.join(path).is_file())
        .collect();
    if !surviving.is_empty() {
        let status = Command::new("git")
            .current_dir(root)
            .args(["add", "--"])
            .args(surviving)
            .status()?;
        if !status.success() {
            return Ok(ExitCode::FAILURE);
        }
    }
    if !check
        .status()
        .inspect_err(|error| eprintln!("{error}"))
        .is_ok_and(|status| status.success())
    {
        println!("{CHECK_FAILURE}");
        return Ok(ExitCode::FAILURE);
    }
    if let Some(smoke) = smoke.filter(|_| needs_smoke(&paths)) {
        println!("Running browser smoke check...");
        if !smoke
            .status()
            .inspect_err(|error| eprintln!("{error}"))
            .is_ok_and(|status| status.success())
        {
            println!("❌ Browser smoke check failed.");
            return Ok(ExitCode::FAILURE);
        }
    }
    println!("✅ All pre-commit checks passed!");
    Ok(ExitCode::SUCCESS)
}

/// Check whether captured paths affect browser compilation.
fn needs_smoke(paths: &[OsString]) -> bool {
    paths.iter().any(|path| {
        let path = Path::new(path);
        path == Path::new("Cargo.toml")
            || path == Path::new("Cargo.lock")
            || path.starts_with("crates/maestro-models")
            || path.starts_with("crates/maestro-web")
    })
}

/// Capture staged paths without losing native filename bytes.
fn capture(root: &Path) -> io::Result<Vec<OsString>> {
    let captured = Command::new("git")
        .current_dir(root)
        .args(["diff", "--cached", "--name-only", "-z"])
        .output()?;
    if !captured.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&captured.stderr).into_owned(),
        ));
    }
    let bytes = captured
        .stdout
        .split(|byte| *byte == 0)
        .filter(|bytes| !bytes.is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Ok(bytes
            .map(|bytes| OsString::from_vec(bytes.to_vec()))
            .collect())
    }
    #[cfg(not(unix))]
    {
        bytes
            .map(|bytes| {
                String::from_utf8(bytes.to_vec())
                    .map(OsString::from)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
            })
            .collect()
    }
}
