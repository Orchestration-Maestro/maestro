//! Scenario harness that drives the terminal toolkit through its public interface.
//!
//! The library holds no code: the recording terminal, the emulated terminal and the
//! manual runtime live beside the integration tests in `tests/support`, so no product
//! crate can depend on them and the terminal emulator stays a development dependency.
