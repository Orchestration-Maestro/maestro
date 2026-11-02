//! Isolated test execution on a fresh Windows console.

use std::os::windows::process::CommandExt;
use std::process::{Command, Output, Stdio};

use crate::{TestProcess, TestProcessError};

/// Marks the console host and the scenario process.
const ROLE: &str = "MAESTRO_TEST_PROCESS_CONSOLE_ROLE";

/// Proves that the scenario returned rather than exiting early.
const DONE: &str = "maestro console scenario finished";

/// Gives the host its own console, independent of the test runner.
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

/// Run `scenario` in a copy of the calling test attached to a fresh console.
///
/// `configure` runs in the console host and chooses the scenario's standard handles.
/// Standard error stays captured for diagnostics and completion proof. The calling
/// process checks both exit status and completion, so an early successful exit fails.
///
/// # Errors
///
/// Fails when the test cannot be selected, launched or proven to have finished, or
/// when the console host or scenario fails.
pub fn on_console(
    test: &str,
    configure: impl FnOnce(&mut Command) -> &mut Command,
    scenario: impl FnOnce(),
) -> Result<(), TestProcessError> {
    let name = TestProcess::current(test)?.name().to_owned();
    let output = match std::env::var(ROLE).as_deref() {
        Ok("scenario") => {
            scenario();
            eprintln!("{DONE}");
            return Ok(());
        }
        Ok("host") => copy(&name, "scenario", configure)?,
        _ => copy(&name, "host", |command| {
            command.creation_flags(CREATE_NEW_CONSOLE)
        })?,
    };
    let report = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() || !report.lines().any(|line| line == DONE) {
        return Err(TestProcessError::new(
            &name,
            format!(
                "console scenario failed or did not finish ({})",
                output.status
            ),
        )
        .with_streams(&output.stdout, &output.stderr));
    }
    eprint!("{report}");
    Ok(())
}

/// Run the selected test as `role`, capturing diagnostics and leaving handles to its caller.
fn copy(
    name: &str,
    role: &str,
    configure: impl FnOnce(&mut Command) -> &mut Command,
) -> Result<Output, TestProcessError> {
    TestProcess::named(name)?
        .nocapture()
        .single_threaded()
        .invoke_with(|arguments| {
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args(arguments)
                .env(ROLE, role)
                .stdin(Stdio::null())
                .stdout(Stdio::null());
            configure(&mut command).stderr(Stdio::piped()).output()
        })
}
