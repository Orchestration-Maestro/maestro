use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Temporarily set authentication aside and restore it after the child.
///
/// # Errors
/// Returns native process and filesystem failures.
pub(crate) fn run(home: &Path, child: &mut Command) -> io::Result<ExitCode> {
    let auth = home.join(".maestro/agent/auth.json");
    let mut backup = None;
    let result = run_without_auth(&auth, &mut backup, child);
    if let Some(backup) = backup {
        move_file(&backup, &auth)?;
        println!("Restored auth.json");
    }
    result
}

/// Back up authentication and run the child without provider credentials.
fn run_without_auth(
    auth: &Path,
    backup: &mut Option<PathBuf>,
    child: &mut Command,
) -> io::Result<ExitCode> {
    if auth.is_file() {
        *backup = Some(move_file(auth, &auth.with_extension("json.bak"))?);
        println!("Moved auth.json to backup");
    }
    child
        .env("MAESTRO_NO_LOCAL_LLM", "1")
        .env_remove("BEDROCK_EXTENSIVE_MODEL_TEST");
    for credential in super::CREDENTIALS {
        child.env_remove(credential);
    }
    println!("Running tests without API keys...");
    let status = child.status()?;
    Ok(super::exit_code(status))
}

/// Move a file, resolving a directory destination to its original filename.
fn move_file(source: &Path, destination: &Path) -> io::Result<PathBuf> {
    let destination = if destination.is_dir() {
        destination.join(
            source
                .file_name()
                .ok_or_else(|| io::Error::other("missing file name"))?,
        )
    } else {
        destination.to_path_buf()
    };
    fs::rename(source, &destination)?;
    Ok(destination)
}
