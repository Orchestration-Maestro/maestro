//! The recorded date-code fields shared by the parser and writer witnesses.
use maestro_app::browser::ssf::DateCode;
use serde::Deserialize;

/// The complete date-code record, keeping the oracle's field spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DateObservation {
    #[serde(rename = "D")]
    serial_day: i32,
    #[serde(rename = "T")]
    seconds: i32,
    #[serde(rename = "u")]
    fraction: f64,
    #[serde(rename = "y")]
    year: i32,
    #[serde(rename = "m")]
    month: u32,
    #[serde(rename = "d")]
    day: u32,
    #[serde(rename = "H")]
    hour: u32,
    #[serde(rename = "M")]
    minute: u32,
    #[serde(rename = "S")]
    second: u32,
    #[serde(rename = "q")]
    weekday: u32,
}

impl From<DateObservation> for DateCode {
    fn from(date: DateObservation) -> Self {
        Self {
            serial_day: date.serial_day,
            seconds: date.seconds,
            fraction: date.fraction,
            year: date.year,
            month: date.month,
            day: date.day,
            hour: date.hour,
            minute: date.minute,
            second: date.second,
            weekday: date.weekday,
        }
    }
}
