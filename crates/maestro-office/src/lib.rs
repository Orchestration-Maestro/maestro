//! Office document reading: spreadsheet number formatting and workbook reading, and Word package reading.

mod decimal;
pub mod docx_preview;
mod links;
pub mod ssf;

pub use decimal::strip_decimal;
pub use links::{executable, script_scheme};
