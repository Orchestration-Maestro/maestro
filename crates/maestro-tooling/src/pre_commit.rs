use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

use selection::Scope;

mod selection;

/// Failure message printed when formatting or verification fails.
const CHECK_FAILURE: &str = "❌ Checks failed. Please fix the errors before committing.";

/// Format captured index paths, restage surviving files and verify their crates.
///
/// Verification covers the staged crates and every workspace crate that depends
/// on them, or the whole workspace when a staged path changes a workspace-wide
/// input. The full conventions suite runs only when a captured path belongs to
/// the conventions checker, and the browser smoke check only when the selection
/// compiles the delivered browser entry.
///
/// # Errors
/// Returns native process and filesystem failures.
pub fn run(
    root: &Path,
    cargo: &Path,
    format: &mut Command,
    conventions: &mut Command,
    smoke: Option<&mut Command>,
) -> io::Result<ExitCode> {
    let paths = capture(root)?;
    println!("Running formatting, linting, and type checking...");
    if !passed(format) {
        println!("{CHECK_FAILURE}");
        return Ok(ExitCode::FAILURE);
    }
    if !restage(root, &paths)? {
        return Ok(ExitCode::FAILURE);
    }
    let Some(scope) = selected(root, cargo, &paths) else {
        println!("{CHECK_FAILURE}");
        return Ok(ExitCode::FAILURE);
    };
    println!("Selected crates: {}", scope.names().join(", "));
    if !passed(&mut check(root, &scope)) {
        println!("{CHECK_FAILURE}");
        return Ok(ExitCode::FAILURE);
    }
    if needs_conventions(&paths)
        && !step(
            "Running the full conventions suite...",
            conventions,
            "❌ Conventions suite failed.",
        )
    {
        return Ok(ExitCode::FAILURE);
    }
    if let Some(smoke) = smoke.filter(|_| scope.reaches_browser())
        && !step(
            "Running browser smoke check...",
            smoke,
            "❌ Browser smoke check failed.",
        )
    {
        return Ok(ExitCode::FAILURE);
    }
    println!("✅ All pre-commit checks passed!");
    Ok(ExitCode::SUCCESS)
}

/// Read locked metadata and select the crates the captured paths require.
fn selected(root: &Path, cargo: &Path, paths: &[OsString]) -> Option<Scope> {
    crate::locked_metadata(cargo, root)
        .map(|metadata| selection::select(&metadata, paths))
        .inspect_err(|error| eprintln!("{error}"))
        .ok()
}

/// Build the shared check recipe for the selected scope.
fn check(root: &Path, scope: &Scope) -> Command {
    let mut command = Command::new("just");
    command.current_dir(root).arg("_check");
    match scope {
        Scope::Workspace(_) => {
            command.arg("--workspace");
        }
        Scope::Packages(names) => {
            for name in names {
                command.args(["-p", name]);
            }
        }
    }
    command
}

/// Stage the captured paths that formatting left on disk.
fn restage(root: &Path, captured: &[OsString]) -> io::Result<bool> {
    let surviving: Vec<_> = captured
        .iter()
        .filter(|path| root.join(path).is_file())
        .collect();
    if surviving.is_empty() {
        return Ok(true);
    }
    let status = Command::new("git")
        .current_dir(root)
        .args(["add", "--"])
        .args(surviving)
        .status()?;
    Ok(status.success())
}

/// Run a step, reporting a launch failure, and tell whether it succeeded.
fn passed(command: &mut Command) -> bool {
    command
        .status()
        .inspect_err(|error| eprintln!("{error}"))
        .is_ok_and(|status| status.success())
}

/// Announce and run a path-selected step, printing `failure` when it does not pass.
fn step(announcement: &str, command: &mut Command, failure: &str) -> bool {
    println!("{announcement}");
    let succeeded = passed(command);
    if !succeeded {
        println!("{failure}");
    }
    succeeded
}

/// Check whether captured paths change the conventions checker or its fixtures.
fn needs_conventions(paths: &[OsString]) -> bool {
    paths.iter().any(|path| is_conventions(Path::new(path)))
}

/// Whether a staged path changes the workspace-wide conventions checker.
fn is_conventions(path: &Path) -> bool {
    path.starts_with("crates/maestro-test-conventions")
}

/// Capture staged paths without losing native filename bytes or move sources.
fn capture(root: &Path) -> io::Result<Vec<OsString>> {
    let captured = Command::new("git")
        .current_dir(root)
        .args(["diff", "--cached", "--name-only", "--no-renames", "-z"])
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
