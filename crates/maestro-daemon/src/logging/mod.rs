//! Structured events and synchronized rotating file output.

mod events;
mod writer;

pub use events::*;

use crate::paths::DaemonPaths;
use std::io;
use tracing_subscriber::EnvFilter;

/// Maximum current log size before replacement.
const DEFAULT_MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
/// Production logs retain no numbered generations.
const DEFAULT_RETAINED_LOG_FILES: usize = 0;

/// Initialize process logging for a supplied role and observed `MAESTRO_LOG` value.
/// File creation and already-initialized subscriber failures leave the caller running.
pub fn init_file_logging(paths: &DaemonPaths, role: &str, filter: Option<&str>) {
    if let Ok(subscriber) = file_subscriber(paths, role, filter) {
        use tracing_subscriber::util::SubscriberInitExt;
        let _ = subscriber.try_init();
    }
}

/// Build the retained file subscriber without installing process-global state.
fn file_subscriber(
    paths: &DaemonPaths,
    role: &str,
    filter: Option<&str>,
) -> io::Result<impl tracing::Subscriber + Send + Sync + use<>> {
    let path = std::path::PathBuf::from(paths.log_path(role));
    let writer = writer::RotatingFileMakeWriter::new(
        path.parent().unwrap_or_else(|| std::path::Path::new("")),
        &format!("maestro-{role}.log"),
        DEFAULT_MAX_LOG_BYTES,
        DEFAULT_RETAINED_LOG_FILES,
    )?;
    let filter = filter
        .and_then(|value| EnvFilter::try_new(value).ok())
        .unwrap_or_else(|| EnvFilter::new("maestro=info"));
    Ok(tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .finish())
}

/// Summarize all role log locations for command help.
#[must_use]
pub fn help_log_paths_summary(paths: &DaemonPaths) -> String {
    format!(
        "{} (plus maestro-client.log, maestro-daemon.log)",
        paths.log_path("in-process")
    )
}

#[cfg(test)]
pub(crate) mod tests;
