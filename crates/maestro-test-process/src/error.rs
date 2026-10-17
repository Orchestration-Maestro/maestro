//! The failure of a checked test process.

use std::error::Error;
use std::fmt;

/// A checked test process could not be selected, started or proven to have started.
///
/// It names the selected test, describes the failed step and keeps the harness
/// diagnostics that were available (captured standard output and standard error) and the
/// native error that caused the failure, when there was one.
#[derive(Debug)]
pub struct TestProcessError {
    /// Full libtest identity the launch selected.
    test: String,
    /// The step that failed.
    message: String,
    /// Standard output observed before the failure.
    stdout: Vec<u8>,
    /// Standard error observed before the failure.
    stderr: Vec<u8>,
    /// The native error behind the failure.
    source: Option<Box<dyn Error>>,
}

impl TestProcessError {
    /// Describe a failed step for the selected test.
    pub(crate) fn new(test: &str, message: impl Into<String>) -> Self {
        Self {
            test: test.to_owned(),
            message: message.into(),
            stdout: Vec::new(),
            stderr: Vec::new(),
            source: None,
        }
    }

    /// Keep the native error that caused the failure.
    pub(crate) fn caused_by(mut self, source: impl Error + 'static) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    /// Keep the harness diagnostics observed so far.
    pub(crate) fn with_streams(mut self, stdout: &[u8], stderr: &[u8]) -> Self {
        self.stdout = stdout.to_vec();
        self.stderr = stderr.to_vec();
        self
    }
}

impl fmt::Display for TestProcessError {
    /// Write the identity, the failed step and any captured diagnostics.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.test, self.message)?;
        if let Some(source) = &self.source {
            write!(f, ": {source}")?;
        }
        for (label, bytes) in [("stdout", &self.stdout), ("stderr", &self.stderr)] {
            if !bytes.is_empty() {
                write!(f, "\n{label}:\n{}", String::from_utf8_lossy(bytes))?;
            }
        }
        Ok(())
    }
}

impl Error for TestProcessError {
    /// Return the native error behind the failure.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref()
    }
}
