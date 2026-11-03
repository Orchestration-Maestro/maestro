//! Native repository development commands.

use maestro_tooling as tooling;

fn main() -> std::process::ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let manifest =
        std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into());
    let checkout = std::path::Path::new(&manifest).join("../..");
    let result = if arguments.first().is_some_and(|name| name == "browser-test") {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .and_then(|runtime| {
                runtime.block_on(tooling::browser_driver::run(
                    &arguments[1..],
                    std::path::Path::new(env!("CARGO")),
                    &checkout,
                ))
            })
    } else {
        tooling::run(&arguments, std::path::Path::new(env!("CARGO")), &checkout)
    };
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
