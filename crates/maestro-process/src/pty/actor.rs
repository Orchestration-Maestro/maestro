//! Ordered asynchronous terminal I/O and ownership handoff.
mod controls;
mod runner;
mod unix;

pub use unix::{PtyIoActor, PtyIoActorConfig, PtyIoActorHandle, PtyReadResult};

#[cfg(test)]
mod tests;
