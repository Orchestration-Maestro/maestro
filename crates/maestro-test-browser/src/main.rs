//! Wiring for pinned-browser qualification.

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .and_then(|runtime| {
            runtime.block_on(maestro_test_browser::browser_driver::run(
                &arguments,
                std::path::Path::new(env!("CARGO")),
                &checkout,
            ))
        });
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
