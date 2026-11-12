//! Calendar and clock acceptance with timezone-aware leap seconds.

use super::matches;

/// Check ordered integer duration units and the separate week form.
pub(super) fn duration(value: &str) -> bool {
    matches(
        r"^P((\d+Y(\d+M(\d+D)?)?|\d+M(\d+D)?|\d+D)(T(\d+H(\d+M(\d+S)?)?|\d+M(\d+S)?|\d+S))?|T(\d+H(\d+M(\d+S)?)?|\d+M(\d+S)?|\d+S)|\d+W)$",
        value,
        "",
    )
}

/// Accept only a valid four-digit-year Gregorian date.
pub(super) fn date(value: &str) -> bool {
    date_components(value).is_some()
}

/// Parse fixed-width date fields through the native calendar validator.
fn date_components(value: &str) -> Option<chrono::NaiveDate> {
    let regex = regress::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})$").ok()?;
    let matched = regex.find(value)?;
    let mut parts = value.get(matched.range)?.split('-');
    chrono::NaiveDate::from_ymd_opt(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    )
}

/// Require both calendar validity and a zoned clock separated by T or t.
pub(super) fn date_time(value: &str) -> bool {
    let Some((date_part, time_part)) = value.split_once(['T', 't']) else {
        return false;
    };
    date(date_part) && time(time_part)
}

/// Reject malformed clocks while retaining valid timezone-adjusted leap seconds.
pub(super) fn time(value: &str) -> bool {
    clock(value).unwrap_or(false)
}

/// Parse local clock fields and validate leap seconds after offset adjustment.
fn clock(value: &str) -> Option<bool> {
    let regex = regress::Regex::with_flags(
        r"^(\d\d):(\d\d):(\d\d(?:\.\d+)?)(?:Z|([+-])(\d\d):(\d\d))$",
        "i",
    )
    .ok()?;
    let matched = regex.find(value)?;
    let value = value.get(matched.range)?;
    let (clock, zone) = if value.ends_with(['z', 'Z']) {
        (value.get(..value.len() - 1)?, None)
    } else {
        let position = value.find(['+', '-'])?;
        (value.get(..position)?, Some(value.get(position..)?))
    };
    let mut components = clock.split(':');
    let hour: i32 = components.next()?.parse().ok()?;
    let minute: i32 = components.next()?.parse().ok()?;
    let second: f64 = components.next()?.parse().ok()?;
    if hour > 23 || minute > 59 {
        return Some(false);
    }
    let (offset_hour, offset_minute, sign) = offset(zone)?;
    if offset_hour > 23 || offset_minute > 59 {
        return Some(false);
    }
    if second < 60.0 {
        return Some(true);
    }
    let utc_minute = minute - offset_minute * sign;
    let utc_hour = hour - offset_hour * sign - i32::from(utc_minute < 0);
    Some(matches!(utc_hour, 23 | -1) && matches!(utc_minute, 59 | -1) && second < 61.0)
}

/// Extract signed hour/minute offsets, treating Z as zero.
fn offset(zone: Option<&str>) -> Option<(i32, i32, i32)> {
    match zone {
        None => Some((0, 0, 1)),
        Some(zone) => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let (hour, minute) = zone.get(1..)?.split_once(':')?;
            Some((hour.parse().ok()?, minute.parse().ok()?, sign))
        }
    }
}
