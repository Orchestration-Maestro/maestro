//! Pure brand pack admission and presentation projection.
//!
//! Native loading belongs to the consumer. See the shared interface in `docs/brand.md`.
mod css;
mod error;
mod mark;
mod pack;
mod presentation;

pub use css::font_stack;
pub use error::BrandError;
pub use pack::{BrandFont, BrandMark, BrandType, PackData};
pub use presentation::{BrandGlyph, BrandMode, BrandPresentation, role_color};
