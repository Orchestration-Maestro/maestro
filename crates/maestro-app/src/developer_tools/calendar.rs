//! Calendar days and local midnights as the reports read them.

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeZone};
use maestro_session::messages::local_timestamp_millis;

/// The calendar day an instant falls on, written as `YYYY-MM-DD`.
///
/// Reports that group by a reported instant use [`utc_day`]; reports that group
/// by the user's own day use [`local_day`] and read the date fields after
/// projecting the instant back into local time.
pub(crate) struct Day(NaiveDate);

impl Day {
    /// The local calendar day containing `millis`.
    pub(crate) fn local(millis: i64) -> Option<Self> {
        Local
            .timestamp_millis_opt(millis)
            .single()
            .map(|date| Self(date.date_naive()))
    }

    /// The UTC calendar day containing `millis`.
    pub(crate) fn utc(millis: i64) -> Option<Self> {
        DateTime::from_timestamp_millis(millis).map(|date| Self(date.date_naive()))
    }

    /// Write `date` the way a report row labels it.
    pub(crate) fn text(date: NaiveDate) -> String {
        Self(date).to_string()
    }
}

impl std::fmt::Display for Day {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (year, month, day) = (self.0.year(), self.0.month(), self.0.day());
        write!(output, "{year:04}-{month:02}-{day:02}")
    }
}

/// The first instant of `date` in local time.
///
/// A day that starts inside a skipped local hour uses the reading's own offset,
/// so the window still begins at the first instant the zone reports for it.
pub(crate) fn local_midnight(date: NaiveDate) -> i64 {
    let Some(time) = date.and_hms_opt(0, 0, 0) else {
        return 0;
    };
    local_timestamp_millis(time).unwrap_or_else(|| time.and_utc().timestamp_millis())
}

/// The local date `days` calendar days before `today`.
pub(crate) fn days_before(today: NaiveDate, days: u32) -> NaiveDate {
    today
        .checked_sub_days(Days::new(u64::from(days)))
        .unwrap_or(today)
}

/// The local date `days` calendar days after `today`.
pub(crate) fn days_after(today: NaiveDate, days: u32) -> NaiveDate {
    today
        .checked_add_days(Days::new(u64::from(days)))
        .unwrap_or(today)
}
