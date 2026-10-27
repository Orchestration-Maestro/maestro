//! The native executable wires command-line startup to its application and adapters.
mod host;
mod modes;
mod presentations;

use std::{error::Error, rc::Rc};
use tokio::task::LocalSet;

/// Perform terminal child setup before constructing any application effects.
fn main() {
    #[cfg(target_os = "linux")]
    if let Some(error) = maestro_process::pty::setup_entry() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
    let status = execute().unwrap_or_else(|error| {
        eprintln!("Error: {error}");
        1
    });
    std::process::exit(status);
}

/// Drive the application and every local native effect on one executor.
fn execute() -> Result<i32, Box<dyn Error>> {
    maestro_process::initialize_coding_agent();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let local = Rc::new(LocalSet::new());
    runtime.block_on(local.run_until(host::run(
        std::env::args().skip(1).collect(),
        Rc::clone(&local),
    )))
}
