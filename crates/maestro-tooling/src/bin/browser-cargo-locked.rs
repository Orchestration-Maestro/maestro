//! Forward browser bundler Cargo invocations through locked metadata handling.

fn main() -> std::process::ExitCode {
    let mut args = vec!["browser-author".into(), "cargo".into()];
    args.extend(std::env::args_os().skip(1));
    let cargo = std::env::var_os("MAESTRO_BROWSER_CARGO").unwrap_or_else(|| env!("CARGO").into());
    match maestro_tooling::run(
        &args,
        std::path::Path::new(&cargo),
        std::path::Path::new("."),
    ) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}
