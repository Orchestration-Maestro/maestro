//! Fixtures and report runners shared by the native analysis reports.

use crate::isolated_home::with_environment;
use chrono::TimeZone;
use maestro_app::developer_tools::{cost, session_transcripts, stats, tool_stats};
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
    let directory = maestro_session::sessions_dir(&home(base, "sessions"), cwd);
    let directory = directory.as_str();
    write_files(directory, files);
    directory.to_owned()
}

/// The working directory `cwd` as the usage report resolves it: `/project` is `D:\\project`
/// on Windows.
pub fn resolved(cwd: &str) -> String {
    maestro_path::resolve_with(
        &[&[cwd]],
        maestro_resources::drive_directories,
        maestro_resources::current_directory,
    )
    .expect("the process directory is readable")
    .remove(0)
}

/// A directory beneath `base`'s home, created.
pub fn home(base: &str, relative: &str) -> String {
    let text = maestro_path::join(&[base, "home/.maestro/agent", relative]);
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

/// Replace the `{BASE}` placeholder in a recorded expectation.
pub fn expected(template: &str, base: &str) -> String {
    template.replace("{BASE}", base)
}

/// Run one report test with an isolated home and no platform viewer.
///
/// Every report test that opens its page runs here. The report's opener is the
/// recorded fake [`run_tools`] installs, so no test starts a real browser, and a
/// test reads the path through [`opened`].
pub fn with_controlled_viewer(test: &str, body: impl FnOnce(&str)) {
    with_controlled_viewer_and(test, &[], body);
}

/// Run one report test as [`with_controlled_viewer`] does, with `variables` added.
pub fn with_controlled_viewer_and(test: &str, variables: &[(&str, &str)], body: impl FnOnce(&str)) {
    let mut selected = vec![("PATH", "/usr/bin:/bin")];
    selected.extend_from_slice(variables);
    with_environment(test, &selected, body);
}

/// The paths the report opener received in this test process, in order.
static OPENED: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();

/// The paths the report opener received in this test process, in order.
#[must_use]
pub fn opened() -> Vec<String> {
    OPENED
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .expect("the opener record is readable")
        .clone()
}

/// Record one path the report handed to the opener.
fn record(path: &str) {
    OPENED
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .expect("the opener record is writable")
        .push(path.to_owned());
}

/// Run the tool report with the fixture clock.
///
/// # Errors
/// Returns the report's own rejection.
pub fn run_tools(values: &[&str]) -> std::io::Result<Run> {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let outcome = tool_stats::main_with_opener(
        &arguments(values),
        &clock,
        &record,
        &mut stdout,
        &mut stderr,
    )?;
    Ok(Run {
        code: outcome,
        stdout: String::from_utf8(stdout).expect("report output is UTF-8"),
        stderr: String::from_utf8(stderr).expect("report errors are UTF-8"),
    })
}

/// Run the transcript command with the fixture clock and the selected painting.
///
/// # Errors
/// Returns the command's own rejection.
pub fn run_transcripts(values: &[&str], color: bool) -> std::io::Result<Run> {
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let outcome = crate::support::run_io(async {
        session_transcripts::main(&arguments(values), color, &mut stdout, &mut stderr).await
    })?;
    Ok(Run {
        code: outcome,
        stdout: String::from_utf8(stdout).expect("command output is UTF-8"),
        stderr: String::from_utf8(stderr).expect("command errors are UTF-8"),
    })
}

/// The report data a written page embeds.
pub fn page_data(page: &str) -> serde_json::Value {
    let embedded = page
        .split_once("const data=")
        .expect("the page embeds its data")
        .1;
    let document = embedded
        .split_once(";\n")
        .expect("the embedded data ends before the page script")
        .0;
    serde_json::from_str(document).expect("the embedded data is JSON")
}

/// A fresh disposable directory.
pub fn scratch() -> maestro_test_directory::Scratch {
    maestro_test_directory::Scratch::new().expect("a scratch directory is creatable")
}

/// The directory text of `scratch`.
pub fn root(scratch: &maestro_test_directory::Scratch) -> String {
    scratch.path().display().to_string()
}
