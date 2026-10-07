//! Native repository development commands.

use maestro_tooling as tooling;

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match tooling::run(&arguments, std::path::Path::new(env!("CARGO")), &checkout) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
