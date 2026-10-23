//! Existing configuration owners exposed at the application root.

pub use crate::configuration::config::{
    APP_NAME, CONFIG_DIR_NAME, ENV_AGENT_DIR, ENV_SESSION_DIR, VERSION,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::configuration::config::get_agent_dir;
