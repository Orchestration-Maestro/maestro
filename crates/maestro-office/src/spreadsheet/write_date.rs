//! Selected calendar and clock field writing.

use super::{
    DateCode, DateToken, FormatError,
    write_num::{pad, round},
};

/// Render one supplied date observation without tokenizing or applying an epoch.
///
/// # Errors
/// Returns an invalid-date-pattern error for unsupported field widths or names.
pub fn write_date(
    kind: DateToken,
    format: &str,
    date: &DateCode,
    subsecond_digits: u8,
) -> Result<String, FormatError> {
    let width = format.len();
    let year = f64::from(date.year) + f64::from(u16::from(kind == DateToken::BuddhistYear) * 543);
    match kind {
        DateToken::Year | DateToken::BuddhistYear => Ok(if width <= 2 {
            pad(year % 100.0, 2)
        } else {
            pad(year % 10000.0, 4)
        }),
        DateToken::Month if width <= 2 => Ok(pad(f64::from(date.month), width)),
        DateToken::Month => {
            let name = MONTHS
                .get(usize::try_from(date.month.saturating_sub(1)).unwrap_or(usize::MAX))
                .ok_or(FormatError::InvalidDateFormat(kind))?;
            Ok(name[match width {
                3 => 1,
                5 => 0,
                _ => 2,
            }]
            .to_owned())
        }
        DateToken::Day if width <= 2 => Ok(pad(f64::from(date.day), width)),
        DateToken::Day => {
            let name = DAYS
                .get(usize::try_from(date.weekday).unwrap_or(usize::MAX))
                .ok_or(FormatError::InvalidDateFormat(kind))?;
            Ok(name[usize::from(width != 3)].to_owned())
        }
        DateToken::Hour12 if (1..=2).contains(&width) => {
            Ok(pad(f64::from(1 + (date.hour + 11) % 12), width))
        }
        DateToken::Hour24 if (1..=2).contains(&width) => Ok(pad(f64::from(date.hour), width)),
        DateToken::Minute if (1..=2).contains(&width) => Ok(pad(f64::from(date.minute), width)),
        DateToken::Second => seconds(format, date, subsecond_digits),
        DateToken::Elapsed => elapsed(format, date, subsecond_digits),
        DateToken::Era => Ok(pad(year, 1)),
        _ => Err(FormatError::InvalidDateFormat(kind)),
    }
}

/// Initial, abbreviated and full English calendar month names.
const MONTHS: [[&str; 3]; 12] = [
    ["J", "Jan", "January"],
    ["F", "Feb", "February"],
    ["M", "Mar", "March"],
    ["A", "Apr", "April"],
    ["M", "May", "May"],
    ["J", "Jun", "June"],
    ["J", "Jul", "July"],
    ["A", "Aug", "August"],
    ["S", "Sep", "September"],
    ["O", "Oct", "October"],
    ["N", "Nov", "November"],
    ["D", "Dec", "December"],
];

/// Abbreviated and full English weekday names, Sunday first.
const DAYS: [[&str; 2]; 7] = [
    ["Sun", "Sunday"],
    ["Mon", "Monday"],
    ["Tue", "Tuesday"],
    ["Wed", "Wednesday"],
    ["Thu", "Thursday"],
    ["Fri", "Friday"],
    ["Sat", "Saturday"],
];

/// Render the selected seconds scale before date-wide carry is applied by the caller.
fn seconds(format: &str, date: &DateCode, digits: u8) -> Result<String, FormatError> {
    if !matches!(format, "s" | "ss" | ".0" | ".00" | ".000") {
        return Err(FormatError::InvalidDateFormat(DateToken::Second));
    }
    if date.fraction == 0.0 && matches!(format, "s" | "ss") {
        return Ok(pad(f64::from(date.second), format.len()));
    }
    let scale = match digits {
        0 => 1.0,
        1 => 10.0,
        3 => 1000.0,
        _ => 100.0,
    };
    let rounded = round(scale * (f64::from(date.second) + date.fraction));
    let rounded = if rounded >= 60.0 * scale {
        0.0
    } else {
        rounded
    };
    if format == "s" {
        return Ok(pad(rounded / scale, 0));
    }
    let text = pad(rounded, 2 + usize::from(digits));
    if format == "ss" {
        Ok(text[..2].to_owned())
    } else {
        Ok(format!(".{}", &text[2..text.len().min(format.len() + 1)]))
    }
}

/// Render elapsed totals without clock modulo or epoch reapplication.
fn elapsed(format: &str, date: &DateCode, digits: u8) -> Result<String, FormatError> {
    let hours = f64::from(date.serial_day) * 24.0 + f64::from(date.hour);
    let minutes = hours * 60.0 + f64::from(date.minute);
    let seconds = if digits == 0 {
        round(f64::from(date.second) + date.fraction)
    } else {
        f64::from(date.second)
    };
    let value = match format {
        "[h]" | "[hh]" => hours,
        "[m]" | "[mm]" => minutes,
        "[s]" | "[ss]" => minutes * 60.0 + seconds,
        _ => return Err(FormatError::InvalidDateFormat(DateToken::Elapsed)),
    };
    Ok(pad(value, if format.len() == 3 { 1 } else { 2 }))
}
