//! Native repository development commands.

use maestro_tooling as tooling;

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let manifest =
        std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into());
    let checkout = std::path::Path::new(&manifest).join("../..");
    match tooling::run(&arguments, std::path::Path::new(env!("CARGO")), &checkout) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
