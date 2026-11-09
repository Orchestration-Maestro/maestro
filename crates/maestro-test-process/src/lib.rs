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
//! that replaces `HOME` passes the run's agent directory back through [`run_agent_directory`],
//! and a cleared child keeps the one the run's setup exported, so neither recompiles the
//! components the run prepared.
//!
//! This crate alone spells the variable that carries a case into its child, [`CASE`]. A caller
//! selects the role with [`TestProcess::case`] and reads it back through [`selection`], and
//! passes any fixture data the child needs in its own variable.
//!
//! An isolated child that needs its own file creation mode, process-count limit, file-size
//! handling or session names those effects with [`IsolatedSetup`] on the builder and applies
//! them at its entry through [`IsolatedSetup::apply_requested`].
//!
//! For a native backend that owns its operands or raw byte framing, repository preparation
//! supplies the harness-free `native_fixture` example through `MAESTRO_NATIVE_FIXTURE`.
//! [`install_executable`] copies it to caller-owned aliases without opening a writable
//! executable in the runner. These children have no libtest output or argument parsing.

mod agent_directory;
mod alone;
pub mod child;
#[cfg(windows)]
mod console;
pub mod credentials;
mod environment;
mod error;
mod executable;
#[cfg(unix)]
pub mod gate;
pub mod git_command;
mod proof;
pub mod rpc_peer;
mod selection;
#[cfg(unix)]
mod setup;
mod test_process;

pub use agent_directory::{AGENT_DIRECTORY, link_files, run_agent_directory};
pub use alone::run_alone;
#[cfg(windows)]
pub use console::on_console;
pub use environment::{CASE, CaseSelection, local_zone_follows_tz, run_per_environment, selection};
pub use error::TestProcessError;
pub use executable::{install_command, install_executable, install_family};
pub use proof::{CheckedChild, CheckedStdout, HARNESS_START};
pub use selection::test_name;
#[cfg(unix)]
pub use setup::{IsolatedSetup, SetupError, SetupGuard};
pub use test_process::TestProcess;

/// Controlled loopback HTTP fixtures.
pub mod loopback;

/// Caller-owned local executors for native test futures.
pub mod executor;
/// Child build toolchain selection.
pub mod toolchain;

/// Shared subprocess helpers.
pub mod subprocess;

/// Shared probe helpers.
pub mod probe;

/// Shared native helpers.
pub mod native;
