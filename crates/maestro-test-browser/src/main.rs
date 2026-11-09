//! Wiring for the nextest setup that prepares the browser export package.

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match maestro_test_browser::browser_driver::prepare(
        &arguments,
        std::path::Path::new(env!("CARGO")),
        &checkout,
    ) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
