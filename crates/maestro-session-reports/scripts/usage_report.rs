//! Print the recorded usage totals of one project directory.

use maestro_session_reports::stats;
use std::io;
use std::process::ExitCode;

/// Supplies the argument, clock and stream owners to the report.
fn main() -> io::Result<ExitCode> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    stats::main(
        &args,
        &chrono::Local::now,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}
