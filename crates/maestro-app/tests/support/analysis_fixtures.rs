//! Fixtures and report runners shared by the native analysis reports.

use chrono::TimeZone;
use maestro_app::developer_tools::{cost, stats};
use std::path::Path;
use std::process::ExitCode;

/// The instant every report test reads as the current time: noon in New York.
pub const NOW: i64 = 1_773_072_000_000;

/// The zone every report test runs in.
pub const ZONE: &str = "America/New_York";

/// A finished report run.
#[derive(Debug)]
pub struct Run {
    /// Reported exit status.
    pub code: ExitCode,
    /// Reported standard output.
    pub stdout: String,
    /// Reported standard error.
    pub stderr: String,
}

impl Run {
    /// Whether the run reported success.
    pub fn succeeded(&self) -> bool {
        self.code == ExitCode::SUCCESS
    }

    /// Whether the run reported failure.
    pub fn failed(&self) -> bool {
        self.code == ExitCode::from(1)
    }
}

/// The supplied clock reading.
pub fn clock() -> chrono::DateTime<chrono::Local> {
    chrono::Local
        .timestamp_millis_opt(NOW)
        .single()
        .expect("the fixture instant is representable")
}

/// The arguments as an owned vector.
pub fn arguments(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Run the cost report with the fixture clock.
///
/// # Errors
/// Returns the report's own rejection.
pub fn run_cost(values: &[&str]) -> std::io::Result<Run> {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let outcome = cost::main(&arguments(values), &clock, &mut stdout, &mut stderr)?;
    Ok(Run {
        code: outcome,
        stdout: String::from_utf8(stdout).expect("report output is UTF-8"),
        stderr: String::from_utf8(stderr).expect("report errors are UTF-8"),
    })
}

/// Run the usage report with the fixture clock.
///
/// # Errors
/// Returns the report's own rejection.
pub fn run_usage(values: &[&str]) -> std::io::Result<Run> {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let outcome = stats::main(&arguments(values), &clock, &mut stdout, &mut stderr)?;
    Ok(Run {
        code: outcome,
        stdout: String::from_utf8(stdout).expect("report output is UTF-8"),
        stderr: String::from_utf8(stderr).expect("report errors are UTF-8"),
    })
}

/// The sessions directory of `cwd` beneath `base`'s home, created with the files.
pub fn sessions(base: &str, cwd: &str, files: &[(&str, &str)]) -> String {
    let directory = home(base, &format!("sessions/{}", encode(cwd)));
    write_files(&directory, files);
    directory
}

/// A directory beneath `base`'s home, created.
pub fn home(base: &str, relative: &str) -> String {
    let directory = Path::new(base).join("home/.maestro/agent").join(relative);
    let text = directory.display().to_string();
    std::fs::create_dir_all(&text).expect("the fixture directory is creatable");
    text
}

/// Write each named file into `directory`.
pub fn write_files(directory: &str, files: &[(&str, &str)]) {
    std::fs::create_dir_all(directory).expect("the fixture directory is creatable");
    for (name, content) in files {
        std::fs::write(Path::new(directory).join(name), content)
            .expect("the fixture file is writable");
    }
}

/// The session directory name of an authored working directory.
fn encode(cwd: &str) -> String {
    maestro_session::encode_session_dir(cwd)
}

/// Replace the `{BASE}` placeholder in a recorded expectation.
pub fn expected(template: &str, base: &str) -> String {
    template.replace("{BASE}", base)
}
