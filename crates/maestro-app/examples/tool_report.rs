//! Write the interactive tool report for the recorded sessions.
#[cfg(not(target_arch = "wasm32"))]
use maestro_app::developer_tools::tool_stats;
#[cfg(not(target_arch = "wasm32"))]
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::process::ExitCode;

/// Supplies the argument, clock and stream owners to the report.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> io::Result<ExitCode> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    tool_stats::main(
        &args,
        &chrono::Local::now,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

/// The report has no browser entry effects.
#[cfg(target_arch = "wasm32")]
fn main() {}
