//! Browser tool rendering implementations.
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub mod default_renderer;

#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub mod bash_renderer;
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub mod calculate_renderer;
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub(super) mod card_content;
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub(super) mod collapsible_card;
#[cfg(any(target_arch = "wasm32", test, feature = "test-support"))]
pub mod get_current_time_renderer;
