//! Model picker over the catalog and configured custom providers.
#[cfg(target_arch = "wasm32")]
mod dom;
#[cfg(target_arch = "wasm32")]
mod rows;
mod search;
#[cfg(test)]
mod tests;

#[cfg(target_arch = "wasm32")]
pub use dom::ModelSelector;
