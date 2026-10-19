//! Session-bound transcript presentation and reader-follow policy.
#[cfg(any(target_arch = "wasm32", test))]
mod scroll;
#[cfg(target_arch = "wasm32")]
mod session;
#[cfg(target_arch = "wasm32")]
mod viewport;
#[cfg(target_arch = "wasm32")]
pub use session::AgentInterface;
