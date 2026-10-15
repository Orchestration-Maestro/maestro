//! ISO dates, local clocks and instants converted to spreadsheet serials.
use super::WorkbookError;
use num_traits::ToPrimitive;
/// Convert UTC dates or epoch-relative clocks without a synthetic leap month.
pub(super) fn datenum(value: &str, date1904: bool) -> Result<f64, WorkbookError> {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    let clock = ["%H:%M:%S%.f", "%H:%M"]
        .iter()
        .find_map(|f| NaiveTime::parse_from_str(value, f).ok());
    let epoch = if date1904 { "1904-01-01" } else { "1899-12-31" };
    let instant = clock
        .map_or_else(
            || chrono::DateTime::parse_from_rfc3339(value).map(|d| d.naive_utc()),
            |time| NaiveDate::parse_from_str(epoch, "%Y-%m-%d").map(|d| d.and_time(time)),
        )
        .or_else(|_| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d").map(|d| d.and_time(NaiveTime::MIN))
        })
        .or_else(|_| {
            let local = value.replace('T', " ");
            ["%Y-%m-%d %H:%M:%S%.f", "%Y-%m-%d %H:%M"]
                .iter()
                .find_map(|f| NaiveDateTime::parse_from_str(&local, f).ok())
                .ok_or_else(|| WorkbookError::Reader(format!("invalid date {value}")))
        })?;
    let instant = instant.and_utc();
    let milliseconds = instant
        .timestamp_millis()
        .to_f64()
        .ok_or_else(|| WorkbookError::Reader("date outside numeric range".into()))?;
    let serial = (milliseconds + 2_209_161_600_000.0) / 86_400_000.0;
    if date1904 {
        Ok(serial - 1462.0)
    } else {
        Ok(if serial < 61.0 { serial - 1.0 } else { serial })
    }
}
