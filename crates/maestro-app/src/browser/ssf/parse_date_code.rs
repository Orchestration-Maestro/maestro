//! Gregorian spreadsheet serial observations.

use chrono::{Datelike, Days, NaiveDate};
use num_traits::ToPrimitive;

/// Calendar and time observations for a spreadsheet serial.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DateCode {
    /// Normalized serial day before the epoch offset, including midnight carry.
    pub serial_day: i32,
    /// Integral seconds within the day after carry.
    pub seconds: i32,
    /// Remaining subsecond fraction.
    pub fraction: f64,
    /// Calendar year.
    pub year: i32,
    /// Calendar month, one-based.
    pub month: u32,
    /// Day of month, including the day-zero sentinel.
    pub day: u32,
    /// Hour within the day.
    pub hour: u32,
    /// Minute within the hour.
    pub minute: u32,
    /// Second within the minute.
    pub second: u32,
    /// Weekday with Sunday numbered zero.
    pub weekday: u32,
}

/// Parse a Gregorian serial in the standard or 1904 date system.
///
/// Invalid serials return `None`; the standard epoch retains day zero and its synthetic leap day.
#[must_use]
pub fn parse_date_code(value: f64, date1904: bool) -> Option<DateCode> {
    if !value.is_finite() || !(0.0..=2_958_465.0).contains(&value) {
        return None;
    }
    let value = normalize_serial(value)?;
    let mut serial_day = value.to_i32()?;
    let mut seconds = (86_400.0 * (value - f64::from(serial_day)))
        .floor()
        .to_i32()?;
    let mut fraction = 86_400.0 * (value - f64::from(serial_day)) - f64::from(seconds);
    if fraction.abs() < 0.000_001 {
        fraction = 0.0;
    }
    if fraction > 0.9999 {
        fraction = 0.0;
        seconds += 1;
        if seconds == 86_400 {
            seconds = 0;
            serial_day += 1;
        }
    }
    let (year, month, day, weekday) = calendar(serial_day + if date1904 { 1462 } else { 0 })?;
    let time = u32::try_from(seconds).ok()?;
    Some(DateCode {
        serial_day,
        seconds,
        fraction,
        year,
        month,
        day,
        hour: time / 3600,
        minute: time / 60 % 60,
        second: time % 60,
        weekday,
    })
}

/// Apply significant rounding followed by the serial format's text truncation.
fn normalize_serial(value: f64) -> Option<f64> {
    let text = super::general::precision(value, 16, false);
    let (mantissa, exponent) = text
        .split_once('e')
        .map_or((text.as_str(), None), |(m, e)| (m, Some(e)));
    let length = if mantissa.starts_with("0.") { 17 } else { 16 };
    let truncated = if mantissa.contains('.') {
        mantissa.chars().take(length).collect::<String>()
    } else {
        format!(
            "{}{}",
            &mantissa[..mantissa.len().min(15)],
            "0".repeat(mantissa.len().saturating_sub(15))
        )
    };
    match exponent {
        Some(exp) => format!("{truncated}e{exp}").parse().ok(),
        None => truncated.parse().ok(),
    }
}

/// Convert the effective calendar day without consulting a host clock or timezone.
fn calendar(day: i32) -> Option<(i32, u32, u32, u32)> {
    match day {
        0 => Some((1900, 1, 0, 6)),
        60 => Some((1900, 2, 29, 3)),
        _ => {
            let adjusted = if day > 60 { day - 1 } else { day };
            let date = NaiveDate::from_ymd_opt(1900, 1, 1)?
                .checked_add_days(Days::new(u64::try_from(adjusted - 1).ok()?))?;
            let weekday = date.weekday().num_days_from_sunday();
            Some((
                date.year(),
                date.month(),
                date.day(),
                if adjusted < 60 {
                    (weekday + 6) % 7
                } else {
                    weekday
                },
            ))
        }
    }
}
