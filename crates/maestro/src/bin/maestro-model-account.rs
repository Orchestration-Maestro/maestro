//! The standalone model-account executable delegates to its command-line owner.
#[path = "maestro-model-account/console.rs"]
mod console;

use maestro_models::{get_oauth_provider, get_oauth_providers};
use std::{error::Error, sync::Arc};

/// Run the standalone account grammar, not session startup.
fn main() {
    let status = execute().unwrap_or_else(|error| {
        maestro_agent::diagnostics::write_stderr_line(&format!("Error: {error}"));
        1
    });
    std::process::exit(status);
}

/// Install the built-in providers, then supply the native console and the live provider
/// registry to the delivered account entry.
fn execute() -> Result<i32, Box<dyn Error>> {
    maestro_providers::install_built_in_providers();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(runtime.block_on(maestro_cli::model_login::cli::main(
        &std::env::args().skip(1).collect::<Vec<_>>(),
        get_oauth_providers(),
        &get_oauth_provider,
        Arc::new(console::NativeConsole),
    )))
}
