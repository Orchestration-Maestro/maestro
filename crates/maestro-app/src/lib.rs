//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod presentation_data;

pub use maestro_models::{ModelCost, Usage, UsageCost};
pub use maestro_text::markup;

pub use presentation_data::footer_data_provider::ReadonlyFooterDataProvider;
