//! Write the interactive tool report for the recorded sessions.
use maestro_app::developer_tools::tool_stats;
use std::io;
use std::process::ExitCode;

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
