//! Re-execution transport for a caller-provided terminal setup entry.
use super::{Pty, PtyChild, PtyCommand, PtyEnvironment, PtySize};
use std::{ffi::OsString, io, path::PathBuf, process::Command};

/// Executable and arguments that invoke the child-side terminal setup entry.
#[derive(Debug, Clone)]
pub struct PtySetupEntry {
    /// Executable that calls [`setup_entry`] before ordinary argument parsing.
    pub executable: PathBuf,
    /// Entry selector arguments, empty for a composition-root startup entry.
    pub args: Vec<OsString>,
}

impl PtySetupEntry {
    /// Launches a terminal command through this setup entry.
    ///
    /// # Errors
    /// Returns terminal creation, entry launch or target setup failures.
    pub fn spawn(&self, size: PtySize, command: PtyCommand) -> io::Result<(Pty, PtyChild)> {
        super::spawn(size, command, |command| Ok(self.prepare(command)))
    }

    /// Moves native target operands into the setup entry's environment.
    #[must_use]
    pub fn prepare(&self, mut command: PtyCommand) -> PtyCommand {
        let program = std::mem::replace(&mut command.program, self.executable.clone().into());
        let args = std::mem::replace(&mut command.args, self.args.clone());
        let mut transport = vec![
            (OsString::from("MAESTRO_PTY_PROGRAM"), Some(program)),
            (
                "MAESTRO_PTY_COUNT".into(),
                Some(args.len().to_string().into()),
            ),
        ];
        transport.push(("MAESTRO_PTY_ARGV0".into(), command.argv0.take()));
        transport.extend(
            args.into_iter()
                .enumerate()
                .map(|(index, arg)| (format!("MAESTRO_PTY_ARG_{index}").into(), Some(arg))),
        );
        match &mut command.environment {
            PtyEnvironment::Inherit(edits) => {
                edits.extend(transport);
            }
            PtyEnvironment::Replace(values) => {
                for (key, value) in transport {
                    values.retain(|(existing, _)| existing != &key);
                    values.extend(value.map(|value| (key, value)));
                }
            }
        }
        command
    }
}

/// Executes a terminal setup invocation, or returns `None` for ordinary startup.
///
/// The composition root calls this before parsing application arguments. A setup
/// invocation never returns on success; on failure the caller must exit unsuccessfully.
/// The native failure is also written to the parent's setup stream.
#[must_use]
pub fn setup_entry() -> Option<io::Error> {
    let program = std::env::var_os("MAESTRO_PTY_PROGRAM")?;
    Some(match target_command(program) {
        Ok(command) => super::setup_and_exec(command),
        Err(error) => {
            use std::io::Write;
            let _ = io::stderr().write_all(error.to_string().as_bytes());
            error
        }
    })
}

/// Reads native operands without changing the setup process's global environment.
fn target_command(program: OsString) -> io::Result<Command> {
    let count = std::env::var("MAESTRO_PTY_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid terminal argument count",
            )
        })?;
    let mut command = Command::new(program);
    command
        .env_remove("MAESTRO_PTY_PROGRAM")
        .env_remove("MAESTRO_PTY_COUNT")
        .env_remove("MAESTRO_PTY_ARGV0");
    if let Some(argv0) = std::env::var_os("MAESTRO_PTY_ARGV0") {
        use std::os::unix::process::CommandExt;
        command.arg0(argv0);
    }
    for index in 0..count {
        let key = format!("MAESTRO_PTY_ARG_{index}");
        let value = std::env::var_os(&key).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "missing terminal argument")
        })?;
        command.arg(value).env_remove(key);
    }
    Ok(command)
}
