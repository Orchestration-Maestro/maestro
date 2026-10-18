//! Existing configuration owners exposed at the application root.

pub use crate::configuration::config::VERSION;

#[cfg(not(target_arch = "wasm32"))]
pub use crate::configuration::config::get_agent_dir;
