//! Tool output selection and accounting.

pub(crate) mod truncate;
pub use truncate::*;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod render_parts;
pub mod render_utils;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod output_accumulator;
#[cfg(not(target_arch = "wasm32"))]
pub use output_accumulator::*;

pub(crate) mod visual_truncate;
pub use visual_truncate::*;
