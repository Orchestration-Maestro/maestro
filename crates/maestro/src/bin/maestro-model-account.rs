//! The standalone model-account executable delegates to its command-line owner.
#[path = "maestro-model-account/console.rs"]
mod console;

use maestro_app::presentation_data::{get_oauth_provider, get_oauth_providers};
use std::{error::Error, sync::Arc};

/// Run the standalone account grammar, not session startup.
fn main() {
    let status = execute().unwrap_or_else(|error| {
        eprintln!("Error: {error}");
        1
    });
    std::process::exit(status);
}

/// Supply the native console and live provider registry to the delivered account entry.
fn execute() -> Result<i32, Box<dyn Error>> {
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
