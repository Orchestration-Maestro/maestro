//! Scenario harness that drives the terminal toolkit through its public interface.
//!
//! The library owns the recording terminal and the manual runtime every terminal
//! scenario needs, so one implementation serves the harness and the frontends that
//! present toolkit components under controlled effects. The virtual terminal and the
//! written scenarios stay beside the integration tests.

pub mod manual_runtime;
pub mod recording_terminal;
