//! Tool output selection and accounting.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod render_parts;
pub mod render_utils;
pub mod truncate;

#[cfg(not(target_arch = "wasm32"))]
pub mod output_accumulator;

pub mod visual_truncate;
