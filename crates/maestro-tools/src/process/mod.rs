//! Native process helpers: explicit command execution and cancellable delays.

pub(crate) mod bash;
pub(crate) mod bash_executor;
pub(crate) mod exec;
pub(crate) mod sleep;

/// Caller-local direct shell execution.
pub use bash_executor::local;
