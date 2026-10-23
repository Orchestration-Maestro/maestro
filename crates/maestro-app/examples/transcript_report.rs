//! Write the recorded session transcripts and analyze them when asked.
use maestro_app::developer_tools::session_transcripts;
use std::io::{self, IsTerminal, Write as _};
use std::process::ExitCode;

fn main() -> io::Result<ExitCode> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let mut stdout = io::stdout().lock();
    let color = stdout.is_terminal();
    let outcome = runtime.block_on(async {
        let mut errors = io::stderr().lock();
        session_transcripts::main(&args, color, &mut stdout, &mut errors).await
    });
    match outcome {
        Ok(code) => Ok(code),
        Err(error) => {
            let mut errors = io::stderr().lock();
            let _ = writeln!(errors, "{error}");
            Ok(ExitCode::SUCCESS)
        }
    }
}
