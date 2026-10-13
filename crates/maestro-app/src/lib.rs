//! Application layer shared by every frontend.
//!
//! [`presentation_data`] holds the data frontends render but do not own.

pub mod presentation_data;

pub use maestro_models::{ModelCost, Usage, UsageCost};
pub use maestro_text::markup;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_tools::{
    BashOperations, create_local_bash_operations, utils::shell::get_shell_config,
};

pub use presentation_data::footer_data_provider::ReadonlyFooterDataProvider;

pub use maestro_tools::{key_hint, key_text, raw_key_hint};
