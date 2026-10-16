//! Literal trash execution with permanent unlink fallback.
use maestro_app::{ResourceOperations, is_whitespace_scalar, truncate_utf16};
use maestro_tui::tui::LocalFuture;
use std::{error::Error, path::Path, rc::Rc};
/// Observed trash completion, preserving native failures alongside captured diagnostics.
pub enum TrashResult {
    /// A completed child process.
    Exited {
        /// Whether its exit status was successful.
        success: bool,
        /// Captured standard error.
        stderr: String,
    },
    /// A native launch or capture failure.
    Failed {
        /// Original native error.
        error: Box<dyn Error>,
        /// Captured standard error, when available.
        stderr: String,
    },
}
/// Replaceable literal-trash and permanent-unlink effects.
pub trait SessionFileOperations {
    /// Executes only the trash program with these literal arguments.
    fn trash(&self, args: &[String]) -> TrashResult;
    /// Permanently removes the supplied authored file path.
    fn unlink(&self, path: String) -> LocalFuture;
}
/// Successful deletion method.
pub(super) enum DeleteMethod {
    /// Recoverable trash move, including an already-absent file after trash failed.
    Trash,
    /// Permanent removal.
    Unlink,
}
/// Original unlink failure and its composed user-facing diagnostic.
pub(super) struct DeleteFailure {
    /// Original native cause.
    pub(super) error: Box<dyn Error>,
    /// Optional bounded trash diagnostic.
    pub(super) hint: Option<String>,
}
impl std::fmt::Display for DeleteFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(hint) = &self.hint {
            write!(f, " ({hint})")?;
        }
        Ok(())
    }
}
/// Tries literal trash before checking existence or permanently unlinking.
pub(super) async fn delete_session_file(
    path: String,
    operations: Rc<dyn SessionFileOperations>,
    paths: Rc<dyn ResourceOperations>,
) -> Result<DeleteMethod, DeleteFailure> {
    let args = if path.starts_with('-') {
        vec!["--".into(), path.clone()]
    } else {
        vec![path.clone()]
    };
    let trash = operations.trash(&args);
    if matches!(&trash, TrashResult::Exited { success: true, .. })
        || !paths.exists(Path::new(&path))
    {
        return Ok(DeleteMethod::Trash);
    }
    operations
        .unlink(path)
        .await
        .map(|()| DeleteMethod::Unlink)
        .map_err(|error| DeleteFailure {
            error,
            hint: trash_hint(trash),
        })
}
/// Combines native launch and first stderr-line observations before UTF-16 clipping.
fn trash_hint(result: TrashResult) -> Option<String> {
    let (mut parts, stderr) = match result {
        TrashResult::Exited { stderr, .. } => (Vec::new(), stderr),
        TrashResult::Failed { error, stderr } => (vec![error.to_string()], stderr),
    };
    let stderr = stderr.trim_matches(is_whitespace_scalar);
    if !stderr.is_empty() {
        parts.push(stderr.split('\n').next().unwrap_or(stderr).to_owned());
    }
    (!parts.is_empty()).then(|| format!("trash: {}", truncate_utf16(&parts.join(" · "), 200)))
}
/// Selects injected operations or the native terminal adapter.
pub(super) fn operations(
    supplied: Option<Rc<dyn SessionFileOperations>>,
) -> Option<Rc<dyn SessionFileOperations>> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        supplied.or_else(|| Some(Rc::new(NativeOperations)))
    }
    #[cfg(target_arch = "wasm32")]
    {
        supplied
    }
}
/// Native effects reuse the application's existing capture lifecycle.
#[cfg(not(target_arch = "wasm32"))]
struct NativeOperations;
#[cfg(not(target_arch = "wasm32"))]
impl SessionFileOperations for NativeOperations {
    fn trash(&self, args: &[String]) -> TrashResult {
        use maestro_app::{OutputMode, SpawnSyncError, SpawnSyncOptions, spawn_sync};
        let mut command = tokio::process::Command::new("trash");
        command.args(args);
        let result = spawn_sync(
            &mut command,
            SpawnSyncOptions::Capture {
                stdout: OutputMode::Capture,
                stderr: OutputMode::Capture,
                input: None,
                timeout: None,
                max_buffer_bytes: None,
            },
        );
        match result {
            Ok(output) => TrashResult::Exited {
                success: output.status.success(),
                stderr: maestro_app::decode_utf8(&output.stderr).into_owned(),
            },
            Err(error) => {
                let error: Box<dyn Error> = match error {
                    SpawnSyncError::Infrastructure(error)
                    | SpawnSyncError::Spawn(error)
                    | SpawnSyncError::Wait(error) => Box::new(error),
                    other @ SpawnSyncError::Timeout => Box::new(other),
                };
                TrashResult::Failed {
                    error,
                    stderr: String::new(),
                }
            }
        }
    }
    fn unlink(&self, path: String) -> LocalFuture {
        Box::pin(async move {
            std::fs::remove_file(path)?;
            Ok(())
        })
    }
}
