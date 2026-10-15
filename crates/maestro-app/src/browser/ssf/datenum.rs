//! ISO instants converted to spreadsheet serials.
use super::WorkbookError;
use num_traits::ToPrimitive;
/// Convert a real UTC instant without treating February 1900 as a leap month.
pub(super) fn datenum(value: &str, date1904: bool) -> Result<f64, WorkbookError> {
    let instant = chrono::DateTime::parse_from_rfc3339(value)
        .map_err(|e| WorkbookError::Reader(format!("invalid date {value}: {e}")))?;
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
