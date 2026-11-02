//! Controlled native child processes for tests.
//!
//! A test that re-runs itself, or a named sibling, in a fresh process starts it through
//! [`TestProcess`]. The owner resolves one exact libtest identity, confirms the test is
//! eligible to run, and requires the child's standard output to begin with the harness
//! start prologue for exactly one test. A stale name therefore fails instead of passing
//! with zero tests. A checked outcome says the selected body started, not that it passed:
//! callers keep their exit, signal and product assertions. [`TestProcess::invoke_with`]
//! checks eligibility only; its caller proves startup through its own transport. A test that closes a pipe end
//! while a child is live runs alone through [`run_alone`]. Cases that read process variables
//! share one cleared child per distinct environment through [`run_per_environment`]. A child
//! that replaces `HOME` keeps this process's Linux user cache through [`user_cache_home`].
//!
//! For a native backend that owns its operands or raw byte framing, repository preparation
//! supplies the harness-free `native_fixture` example through `MAESTRO_NATIVE_FIXTURE`.
//! [`install_executable`] copies it to caller-owned aliases without opening a writable
//! executable in the runner. These children have no libtest output or argument parsing.

mod alone;
mod cache;
pub mod child;
#[cfg(windows)]
mod console;
mod environment;
mod error;
mod executable;
mod proof;
pub mod rpc_peer;
mod selection;
mod test_process;

pub use alone::run_alone;
pub use cache::user_cache_home;
#[cfg(windows)]
pub use console::on_console;
pub use environment::{CaseSelection, local_zone_follows_tz, run_per_environment, selection};
pub use error::TestProcessError;
pub use executable::install_executable;
pub use proof::{CheckedChild, CheckedStdout, HARNESS_START};
pub use selection::test_name;
pub use test_process::TestProcess;
