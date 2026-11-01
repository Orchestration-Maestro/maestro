//! Native asynchronous standard operating-system interfaces available to authored extensions.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

pub use crate::bindings::standard::{wasi::*, wit_stream};
