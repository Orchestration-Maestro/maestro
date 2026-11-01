//! Existing text owners exposed at the application root.

pub use maestro_text::markup;

pub use maestro_text::is_whitespace_scalar;

pub use maestro_text::number_format::LocaleNumberFormat;

pub use maestro_text::{decode_utf8, lossy_string};

/// Shared with the terminal frontend.
pub use maestro_text::truncate_utf16;

pub use maestro_text::normalize_to_lf;

/// Shared CSS string serialization.
pub use maestro_text::css;

pub use maestro_text::is_truthy_env_flag;
