//! Replaceable raw-byte shell execution with native local operations.
mod execute;
mod result;
mod tool;
pub use tool::{
    BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolInput, BashToolOptions,
    create_bash_tool, create_bash_tool_definition,
};

use crate::utils::shell::{
    ShellEnv, get_shell_config, get_shell_env, kill_process_tree, track_detached_child_pid,
    untrack_detached_child_pid,
};
use maestro_models::Cancellation;
use maestro_process::child_process::{
    CapturedChild, EXIT_STDIO_GRACE_MS, WaitOptions, wait_for_child_process,
};
use std::{error::Error, fmt, future::Future, io, path::Path, pin::Pin, sync::Arc, time::Duration};

/// Inputs for one raw shell execution.
pub struct BashExecOptions<'a> {
    /// Receives consumed raw stdout and stderr chunks synchronously.
    pub on_data: &'a mut (dyn FnMut(&[u8]) + Send),
    /// Optional cooperative cancellation.
    pub signal: Option<&'a Cancellation>,
    /// Optional positive timeout in seconds.
    pub timeout: Option<f64>,
    /// Complete replacement environment; omission reads fresh inheritance.
    pub env: Option<&'a ShellEnv>,
}

/// The shell's exit result, without narrowing supplied implementations' values.
#[derive(Debug)]
pub struct BashExecResult {
    /// Numeric status or no status when a signal ended the process.
    pub exit_code: Option<f64>,
}

/// Why a shell operation failed.
#[derive(Debug)]
pub enum BashError {
    /// The cancellation signal was aborted at successful settlement.
    Aborted,
    /// The selected timeout expired before settlement.
    Timeout(f64),
    /// The original native or supplied operation error.
    Other(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for BashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Aborted => f.write_str("aborted"),
            Self::Timeout(seconds) => write!(f, "timeout:{seconds}"),
            Self::Other(error) => error.fmt(f),
        }
    }
}

impl Error for BashError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Other(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<io::Error> for BashError {
    fn from(error: io::Error) -> Self {
        Self::Other(Box::new(error))
    }
}

impl From<maestro_models::DiagnosticErrorInfo> for BashError {
    fn from(error: maestro_models::DiagnosticErrorInfo) -> Self {
        Self::Other(Box::new(error))
    }
}

/// A replaceable operation that streams raw bytes from a shell command.
pub trait BashOperations: Send + Sync {
    /// Executes the literal command in the supplied directory.
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + Send + 'a>>;
}

/// Creates local operations; shell and environment observations happen per execution.
#[must_use]
pub fn create_local_bash_operations(
    bin_dir: String,
    shell_path: Option<String>,
) -> Arc<dyn BashOperations> {
    Arc::new(LocalBash {
        bin_dir,
        shell_path,
    })
}

/// Factory-supplied operands, not cached shell discovery or inheritance.
struct LocalBash {
    /// Caller-resolved tool binary directory.
    bin_dir: String,
    /// Optional explicit executable spelling.
    shell_path: Option<String>,
}

impl BashOperations for LocalBash {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: BashExecOptions<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<BashExecResult, BashError>> + Send + 'a>> {
        Box::pin(self.run(command, cwd, options))
    }
}

impl LocalBash {
    /// Resolves, spawns and waits without adding display or agent-tool policies.
    async fn run(
        &self,
        command: &str,
        cwd: &str,
        options: BashExecOptions<'_>,
    ) -> Result<BashExecResult, BashError> {
        let shell = get_shell_config(self.shell_path.as_deref())?;
        if !Path::new(cwd).exists() {
            return Err(io::Error::other(format!(
                "Working directory does not exist: {cwd}\nCannot execute bash commands."
            ))
            .into());
        }
        let mut builder = tokio::process::Command::new(shell.shell);
        builder
            .args(shell.args)
            .arg(command)
            .current_dir(cwd)
            .env_clear();
        match options.env {
            Some(env) => {
                builder.envs(command_env(env, cfg!(windows)));
            }
            None => {
                builder.envs(command_env(&get_shell_env(&self.bin_dir), cfg!(windows)));
            }
        }
        let mut child = CapturedChild::spawn_detached(builder)?;
        let pid = child.id();
        if let Some(pid) = pid {
            track_detached_child_pid(pid);
        }
        let mut expired = false;
        let output = wait_for_child_process(
            &mut child,
            WaitOptions {
                post_exit_grace: Some(Duration::from_millis(EXIT_STDIO_GRACE_MS)),
                ..WaitOptions::default()
            },
            observe(pid, options.signal, options.timeout, &mut expired),
            Some(options.on_data),
        )
        .await;
        if let Some(pid) = pid {
            untrack_detached_child_pid(pid);
        }
        let output = output?;
        if options.signal.is_some_and(Cancellation::is_aborted) {
            return Err(BashError::Aborted);
        }
        if expired {
            return Err(BashError::Timeout(options.timeout.unwrap_or_default()));
        }
        Ok(BashExecResult {
            exit_code: output.status.code().map(f64::from),
        })
    }
}

/// Observes abort and timeout independently while the waiter owns draining and reaping.
async fn observe(
    pid: Option<u32>,
    signal: Option<&Cancellation>,
    seconds: Option<f64>,
    expired: &mut bool,
) -> maestro_process::child_process::Termination {
    let abort = async {
        match signal {
            Some(signal) => signal.cancelled().await,
            None => std::future::pending().await,
        }
        if let Some(pid) = pid {
            kill_process_tree(pid);
        }
    };
    let timeout = async {
        match seconds
            .filter(|seconds| *seconds > 0.0)
            .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok())
        {
            Some(duration) => tokio::time::sleep(duration).await,
            None => std::future::pending().await,
        }
        *expired = true;
        if let Some(pid) = pid {
            kill_process_tree(pid);
        }
    };
    tokio::join!(abort, timeout);
    std::future::pending().await
}

/// Selects entries for native environment insertion.
fn command_env(env: &ShellEnv, windows: bool) -> Vec<(&std::ffi::OsString, &std::ffi::OsString)> {
    let mut entries: Vec<_> = env.iter().collect();
    if windows {
        entries.sort_by(
            |(left, _), (right, _)| match (left.to_str(), right.to_str()) {
                (Some(left), Some(right)) => maestro_text::utf16_order(left, right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => left.cmp(right),
            },
        );
        let mut seen = std::collections::HashSet::new();
        entries.retain(|(key, _)| {
            seen.insert(
                key.to_str()
                    .map_or_else(|| (*key).clone(), |key| key.to_uppercase().into()),
            )
        });
    }
    entries
}

#[cfg(test)]
mod tests;
