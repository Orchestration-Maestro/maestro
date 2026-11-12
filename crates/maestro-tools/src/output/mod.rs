//! Tool output selection and accounting.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod render_parts;
pub mod render_utils;
pub(crate) mod truncate;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod output_accumulator;

pub(crate) mod visual_truncate;
