//! Tool output selection and accounting.

pub mod truncate;

#[cfg(not(target_arch = "wasm32"))]
pub mod output_accumulator;
