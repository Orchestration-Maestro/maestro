#![doc = include_str!("../../../docs/models/generation.md")]
pub mod generate_models;

pub mod generate_test_image;

// The committed boundary fixture names the descriptor record as `crate::Model`.
#[cfg(test)]
use maestro_interchange::types::Model;
