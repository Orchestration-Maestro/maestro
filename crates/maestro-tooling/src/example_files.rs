//! Source discovery shared by the extension index and component preparation.

/// Source marker traversal shared with component preparation and prepared-artifact lookup.
mod components;
/// Rendering over the source-owned component inventory.
mod preparation;

pub(crate) use components::{EXAMPLE_PACKAGE, FIXED_COMPONENTS, validate_variable};
pub use components::{collect, target_name};
pub use preparation::prepare;
