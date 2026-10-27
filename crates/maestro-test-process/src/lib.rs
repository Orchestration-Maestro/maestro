//! Checked child processes for tests.
//!
//! A test that re-runs itself, or a named sibling, in a fresh process starts it through
//! [`TestProcess`]. The owner resolves one exact libtest identity, confirms the test is
//! eligible to run, and requires the child's standard output to begin with the harness
//! start prologue for exactly one test. A stale name therefore fails instead of passing
//! with zero tests. A checked outcome says the selected body started, not that it passed:
//! callers keep their exit, signal and product assertions. [`TestProcess::invoke_with`]
//! checks eligibility only; its caller proves startup through its own transport. A test that closes a pipe end
//! while a child is live runs alone through [`run_alone`]. Cases that read process variables
//! share one cleared child per distinct environment through [`run_per_environment`].

mod alone;
mod environment;
mod error;
mod proof;
mod selection;
mod test_process;

pub use alone::run_alone;
pub use environment::{CaseSelection, local_zone_follows_tz, run_per_environment, selection};
pub use error::TestProcessError;
pub use proof::{CheckedChild, CheckedStdout};
pub use selection::test_name;
pub use test_process::TestProcess;
