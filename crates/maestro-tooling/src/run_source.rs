use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Platform comparison for the owned environment switch.
#[derive(Clone, Copy)]
pub(crate) enum Mode {
    /// Exact switch comparison.
    Unix,
    /// ASCII case-insensitive switch comparison.
    Windows,
}

impl Mode {
    /// Recognize the environment-removal switch using platform comparison rules.
    fn owns_switch(self, argument: &std::ffi::OsStr) -> bool {
        match self {
            Self::Unix => argument == "--no-env",
            Self::Windows => argument.eq_ignore_ascii_case("--no-env"),
        }
    }
}

/// Launch source while preserving native arguments and caller directory.
pub(crate) fn run(cargo: &Path, checkout: &Path, arguments: &[OsString], mode: Mode) -> ExitCode {
    let mut child = Command::new(cargo);
    child
        .args(["run", "--quiet", "--manifest-path"])
        .arg(checkout.join("Cargo.toml"))
        .args(["-p", "maestro", "--"]);
    let no_env = arguments.iter().any(|argument| mode.owns_switch(argument));
    child.args(
        arguments
            .iter()
            .filter(|argument| !mode.owns_switch(argument)),
    );
    if no_env {
        for credential in super::CREDENTIALS.iter().chain(&[
            "AZURE_OPENAI_API_KEY",
            "AZURE_OPENAI_BASE_URL",
            "AZURE_OPENAI_RESOURCE_NAME",
        ]) {
            child.env_remove(credential);
        }
        println!("Running without API keys...");
    }
    match child.status() {
        Ok(status) => super::exit_code(status),
        Err(error) => {
            match error.kind() {
                io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied => eprintln!(
                    "cargo not found at {}. Run just setup from the repo root first.",
                    cargo.display()
                ),
                _ => eprintln!("{error}"),
            }
            ExitCode::FAILURE
        }
    }
}
