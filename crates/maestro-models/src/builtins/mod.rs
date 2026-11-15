//! Provider credential discovery from the process environment.
/// Credential names and authenticated environment detection.
mod env_api_keys;
#[cfg(not(target_arch = "wasm32"))]
pub use env_api_keys::{env_value, raw_env_value};
pub use env_api_keys::{find_env_keys, get_env_api_key};
