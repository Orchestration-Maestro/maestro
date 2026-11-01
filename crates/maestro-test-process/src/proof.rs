//! The start proof: the harness prologue for exactly one test at the start of stdout.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, ExitStatus, Output};

use crate::TestProcessError;

/// Harness bytes that open the standard output of a run that selected exactly one test.
pub const HARNESS_START: &[u8] = b"\nrunning 1 test\n";

/// Report whether `seen` already contradicts the start prologue.
fn diverges(seen: &[u8]) -> bool {
    !HARNESS_START.starts_with(seen) && !seen.starts_with(HARNESS_START)
}

/// Require a complete start prologue at byte zero of captured output.
pub(crate) fn check_output(test: &str, output: &Output) -> Result<(), TestProcessError> {
    check_prefix(test, &output.stdout, &output.stderr, output.status)
}

/// Require the start prologue at byte zero of `seen`, attaching diagnostics on failure.
pub(crate) fn check_prefix(
    test: &str,
    seen: &[u8],
    stderr: &[u8],
    status: ExitStatus,
) -> Result<(), TestProcessError> {
    if seen.starts_with(HARNESS_START) {
        return Ok(());
    }
    Err(TestProcessError::new(
        test,
        format!("the child did not start exactly this test ({status})"),
    )
    .with_streams(seen, stderr))
}

/// Inspect the start prologue through the initially empty shared open file, leaving its
/// cursor untouched.
pub(crate) fn check_file(
    test: &str,
    mut file: &File,
    status: ExitStatus,
) -> Result<(), TestProcessError> {
    let fail = |error: io::Error| {
        TestProcessError::new(test, "cannot inspect the redirected standard output")
            .caused_by(error)
    };
    let cursor = file.stream_position().map_err(fail)?;
    file.seek(SeekFrom::Start(0)).map_err(fail)?;
    let mut seen = Vec::new();
    let read = file.take(HARNESS_START.len() as u64).read_to_end(&mut seen);
    file.seek(SeekFrom::Start(cursor)).map_err(fail)?;
    read.map_err(fail)?;
    check_prefix(test, &seen, &[], status)
}

/// Standard output of a live child whose start prologue was read and is replayed first.
///
/// Dropping it closes the parent's read end of the pipe.
#[derive(Debug)]
pub struct CheckedStdout {
    /// How much of the prologue the caller has already read.
    replayed: usize,
    /// The child's pipe, positioned just after the prologue.
    inner: ChildStdout,
}

impl Read for CheckedStdout {
    /// Yield the prologue first, then the child's own bytes.
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.replayed < HARNESS_START.len() {
            let count = bytes.len().min(HARNESS_START.len() - self.replayed);
            bytes[..count].copy_from_slice(&HARNESS_START[self.replayed..self.replayed + count]);
            self.replayed += count;
            return Ok(count);
        }
        self.inner.read(bytes)
    }
}

/// A running child whose start was proven before it was handed to the caller.
///
/// The descriptors the builder piped are public; the process handle stays private.
#[derive(Debug)]
pub struct CheckedChild {
    /// The piped standard input, when the builder piped it.
    pub stdin: Option<ChildStdin>,
    /// The piped standard output, replaying the start prologue first.
    pub stdout: Option<CheckedStdout>,
    /// The piped standard error, when the builder piped it.
    pub stderr: Option<ChildStderr>,
    /// The direct child process.
    child: Child,
    /// Full libtest identity the launch selected.
    test: String,
}

impl CheckedChild {
    /// Read only the prologue from the child's standard output and prove it.
    ///
    /// On failure the direct child is closed and reaped.
    pub(crate) fn start(test: &str, mut child: Child) -> Result<Self, TestProcessError> {
        let stdin = child.stdin.take();
        let stderr = child.stderr.take();
        let mut seen = Vec::new();
        let Some(mut pipe) = child.stdout.take() else {
            return Err(Self::reap(
                test,
                child,
                stdin,
                &seen,
                "standard output is not piped",
            ));
        };
        while seen.len() < HARNESS_START.len() {
            let mut byte = [0; 1];
            match pipe.read(&mut byte) {
                Ok(1) => seen.push(byte[0]),
                Ok(_) => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    let failure = Self::reap(test, child, stdin, &seen, "cannot read the start");
                    return Err(failure.caused_by(error));
                }
            }
            if diverges(&seen) {
                break;
            }
        }
        if !seen.starts_with(HARNESS_START) {
            let message = "the child did not start exactly this test";
            return Err(Self::reap(test, child, stdin, &seen, message));
        }
        let stdout = Some(CheckedStdout {
            replayed: 0,
            inner: pipe,
        });
        Ok(Self {
            stdin,
            stdout,
            stderr,
            child,
            test: test.to_owned(),
        })
    }

    /// Close the child's input, stop the direct child and reap it for a failed start.
    fn reap(
        test: &str,
        mut child: Child,
        stdin: Option<ChildStdin>,
        seen: &[u8],
        message: &str,
    ) -> TestProcessError {
        drop(stdin);
        let _ = child.kill();
        match child.wait() {
            Ok(status) => {
                TestProcessError::new(test, format!("{message} ({status})")).with_streams(seen, &[])
            }
            Err(cause) => TestProcessError::new(test, message)
                .with_streams(seen, &[])
                .caused_by(cause),
        }
    }

    /// Close the child's input, then wait for it to exit.
    ///
    /// # Errors
    ///
    /// Fails when the operating system cannot wait for the child.
    pub fn wait(&mut self) -> Result<ExitStatus, TestProcessError> {
        drop(self.stdin.take());
        self.child.wait().map_err(|error| {
            TestProcessError::new(&self.test, "cannot wait for the child").caused_by(error)
        })
    }

    /// Close the child's input and collect its exit status and the bytes the caller has not
    /// yet read from each stream, including an unread part of the prologue.
    ///
    /// # Errors
    ///
    /// Fails when the operating system cannot wait for the child or read its streams.
    pub fn wait_with_output(mut self) -> Result<Output, TestProcessError> {
        drop(self.stdin.take());
        let replayed = self.stdout.as_ref().map(|stdout| stdout.replayed);
        self.child.stderr = self.stderr.take();
        self.child.stdout = self.stdout.take().map(|stdout| stdout.inner);
        let mut output = self.child.wait_with_output().map_err(|error| {
            TestProcessError::new(&self.test, "cannot collect the output").caused_by(error)
        })?;
        if let Some(replayed) = replayed.filter(|replayed| *replayed < HARNESS_START.len()) {
            output
                .stdout
                .splice(0..0, HARNESS_START[replayed..].iter().copied());
        }
        Ok(output)
    }
}
