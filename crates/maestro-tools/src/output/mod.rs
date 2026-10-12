//! Tool output selection and accounting.

pub mod render_utils;
pub mod truncate;

#[cfg(not(target_arch = "wasm32"))]
pub mod output_accumulator;
