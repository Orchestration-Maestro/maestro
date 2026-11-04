//! Native repository development commands.

use std::path::PathBuf;

use maestro_tooling as tooling;

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match tooling::run(&arguments, std::path::Path::new(env!("CARGO")), &checkout()) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// The workspace the command serves: the one this process starts in.
///
/// An installed command is started in the workspace it prepares, so its own source tree names
/// nothing. A directory outside every workspace, which only stages this command, falls back to
/// the tree it was compiled in.
fn checkout() -> PathBuf {
    let start = std::env::current_dir().unwrap_or_default();
    tooling::workspace::root(&start)
        .unwrap_or_else(|_| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
}
