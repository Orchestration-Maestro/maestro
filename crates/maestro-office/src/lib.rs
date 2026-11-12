//! Office document reading: spreadsheet number formatting and workbook reading, and Word package reading.

mod decimal;
mod links;
pub mod spreadsheet;
pub mod wordprocessing;

pub use decimal::strip_decimal;
pub use links::{executable, script_scheme};
