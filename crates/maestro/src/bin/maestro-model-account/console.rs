//! Standard streams for the standalone account command's supplied console interface.
use maestro_app::{BoxFuture, OAuthError};
use maestro_cli::model_login::cli::{AccountConsole, AccountInput};
use std::{
    io::{self, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::io::AsyncBufReadExt;

/// Native output and freshly owned line-input interfaces.
pub struct NativeConsole;
impl AccountConsole for NativeConsole {
    fn write(&self, text: &str, is_error: bool) {
        let result = if is_error {
            io::stderr().write_all(text.as_bytes())
        } else {
            io::stdout().write_all(text.as_bytes())
        };
        if result.is_err() {
            std::process::exit(1);
        }
    }
    fn create_interface(&self) -> Arc<dyn AccountInput> {
        Arc::new(NativeInput {
            reader: Arc::new(tokio::sync::Mutex::new(tokio::io::BufReader::new(
                tokio::io::stdin(),
            ))),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }
}

/// One line reader, released when its command finishes.
struct NativeInput {
    /// Buffered standard input shared only by this interface's pending operation.
    reader: Arc<tokio::sync::Mutex<tokio::io::BufReader<tokio::io::Stdin>>>,
    /// Whether the owning account operation released this interface.
    closed: Arc<AtomicBool>,
}
impl AccountInput for NativeInput {
    fn prompt(&self, question: String) -> BoxFuture<Result<String, OAuthError>> {
        let (reader, closed) = (Arc::clone(&self.reader), Arc::clone(&self.closed));
        Box::pin(async move {
            if closed.load(Ordering::Relaxed) {
                return Err(OAuthError::from(maestro_app::DiagnosticErrorInfo::error(
                    "input interface is closed",
                )));
            }
            let mut output = io::stdout();
            output
                .write_all(question.as_bytes())
                .and_then(|()| output.flush())
                .map_err(|error| {
                    OAuthError::from(maestro_app::DiagnosticErrorInfo::error(error.to_string()))
                })?;
            let mut answer = String::new();
            reader
                .lock()
                .await
                .read_line(&mut answer)
                .await
                .map_err(|error| {
                    OAuthError::from(maestro_app::DiagnosticErrorInfo::error(error.to_string()))
                })?;
            Ok(answer.trim_end_matches(['\r', '\n']).to_owned())
        })
    }
    fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}
