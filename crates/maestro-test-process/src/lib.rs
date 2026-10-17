//! Checked child processes for tests.
//!
//! A test that re-runs itself, or a named sibling, in a fresh process starts it through
//! [`TestProcess`]. The owner resolves one exact libtest identity, confirms the test is
//! eligible to run, and requires the child's standard output to begin with the harness
//! start prologue for exactly one test. A stale name therefore fails instead of passing
//! with zero tests. A checked outcome says the selected body started, not that it passed:
//! callers keep their exit, signal and product assertions.
//!
//! The owner exposes no unchecked `Command`, argument vector or success. Launches still
//! built outside the builder use the `legacy` bridge items ([`capture`], [`rerun`],
//! [`assert_executed`], [`selector`], [`child_case`], [`TestResult`]), which share the same
//! prologue proof.

mod error;
mod legacy;
mod proof;
mod selection;
mod test_process;

pub use error::TestProcessError;
pub use legacy::{TestResult, assert_executed, capture, child_case, rerun, selector};
pub use proof::{CheckedChild, CheckedStdout};
pub use selection::test_name;
pub use test_process::TestProcess;
