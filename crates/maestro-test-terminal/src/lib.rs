//! Scenario harness that drives the terminal toolkit through its public interface.
//!
//! The library reexports the toolkit's recording terminal and manual runtime, so
//! scenarios and frontend witnesses use the same controlled effects. The virtual terminal and the
//! written scenarios stay beside the integration tests.

pub use maestro_tui::test_support::{manual_runtime, recording_terminal};
