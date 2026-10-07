use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, ExitCode};

/// Temporarily set authentication aside and restore it after the child.
///
/// # Errors
/// Returns native process and filesystem failures.
pub(crate) fn run(home: &Path, child: &mut Command) -> io::Result<ExitCode> {
    let auth = home.join(".maestro/agent/auth.json");
    let backup = auth.with_extension("json.bak");
    let result = run_without_auth(&auth, &backup, child);
    let cleanup = if backup.is_file() {
        move_file(&backup, &auth).map(|()| println!("Restored auth.json"))
    } else {
        Ok(())
    };
    match result {
        Err(error) => {
            cleanup?;
            Err(error)
        }
        Ok(status) => {
            cleanup?;
            Ok(status)
        }
    }
}

fn run_without_auth(auth: &Path, backup: &Path, child: &mut Command) -> io::Result<ExitCode> {
    if auth.is_file() {
        move_file(auth, backup)?;
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

fn move_file(source: &Path, destination: &Path) -> io::Result<()> {
    let destination = if destination.is_dir() {
        destination.join(
            source
                .file_name()
                .ok_or_else(|| io::Error::other("missing file name"))?,
        )
    } else {
        destination.to_path_buf()
    };
    fs::rename(source, destination)
}
