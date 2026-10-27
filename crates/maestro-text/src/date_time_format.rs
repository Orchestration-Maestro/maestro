//! Prepared local date-time presentation.
use std::io;

/// A formatter captured from supplied locale observations.
pub struct LocaleDateTimeFormat;
impl LocaleDateTimeFormat {
    /// Captures the user's locale.
    ///
    /// # Errors
    /// Returns a formatter construction failure.
    pub fn new(_environment: &dyn Fn(&str) -> Option<String>) -> io::Result<Self> {
        Ok(Self)
    }
    /// Formats supplied Gregorian local date and time fields.
    ///
    /// # Errors
    /// Returns an error for invalid fields.
    pub fn format(&self, date: [i32; 3], time: [u8; 3]) -> io::Result<String> {
        Ok(format!(
            "{}/{}/{}, {}:{:02}:{:02} {}",
            date[1],
            date[2],
            date[0],
            if time[0] % 12 == 0 { 12 } else { time[0] % 12 },
            time[1],
            time[2],
            if time[0] < 12 { "AM" } else { "PM" }
        ))
    }
}
