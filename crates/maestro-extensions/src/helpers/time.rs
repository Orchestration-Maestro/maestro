//! Host-local wall-clock presentation for authored text.
use chrono::{DateTime, Datelike, Local, NaiveDateTime, TimeZone, Timelike};
use maestro_theme::LocaleDateTimeFormat;

use super::{decode, encode};

/// Reads the host's current local time after accepting no operands.
pub(super) fn local_date_time_string(data: &str) -> Result<String, String> {
    let (): () = decode(data)?;
    encode(&format_local_date_time(
        Local::now().naive_local(),
        &|key| std::env::var(key).ok(),
    )?)
}

/// Formats supplied local fields using the captured environment's locale.
fn format_local_date_time(
    now: NaiveDateTime,
    environment: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let formatter = LocaleDateTimeFormat::new(environment).map_err(|error| error.to_string())?;
    let date = [
        now.year(),
        i32::try_from(now.month()).map_err(|error| error.to_string())?,
        i32::try_from(now.day()).map_err(|error| error.to_string())?,
    ];
    let time = [
        u8::try_from(now.hour()).map_err(|error| error.to_string())?,
        u8::try_from(now.minute()).map_err(|error| error.to_string())?,
        u8::try_from(now.second()).map_err(|error| error.to_string())?,
    ];
    formatter
        .format(date, time)
        .map_err(|error| error.to_string())
}

/// Reads the host's local time of an instant given in milliseconds since the Unix epoch.
///
/// An instant outside the calendar's range reads as `Invalid Date`; malformed operands are errors.
pub(super) fn local_time_string(data: &str) -> Result<String, String> {
    let (millis,): (i64,) = decode(data)?;
    encode(&local_time_in(millis, &Local).unwrap_or_else(|| "Invalid Date".to_owned()))
}

/// Converts an instant through `zone` once and formats its clock fields, or `None` outside
/// the calendar's range.
fn local_time_in<Z: TimeZone>(millis: i64, zone: &Z) -> Option<String> {
    DateTime::from_timestamp_millis(millis)
        .map(|at| format_local_time(at.with_timezone(zone).naive_local()))
}

/// Formats local clock fields with the en-US twelve-hour pattern.
fn format_local_time(at: NaiveDateTime) -> String {
    at.format("%-I:%M:%S %p").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::{
        FixedOffset, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone, Utc,
    };

    use super::{format_local_date_time, format_local_time, local_time_in};

    /// The instant a daylight-saving start moves clocks from 2:00 to 3:00, 2026-03-08T07:00Z.
    const SPRING_FORWARD: i64 = 1_772_953_200_000;

    /// A zone at UTC-5 before `SPRING_FORWARD` and UTC-4 from it on.
    #[derive(Clone)]
    struct Spring;

    impl Spring {
        /// The offset in force at a UTC instant.
        fn offset(utc: &NaiveDateTime) -> FixedOffset {
            let hours = if utc.and_utc().timestamp_millis() < SPRING_FORWARD {
                -5
            } else {
                -4
            };
            FixedOffset::east_opt(hours * 3600).unwrap()
        }
    }

    impl TimeZone for Spring {
        type Offset = FixedOffset;
        fn from_offset(_: &FixedOffset) -> Self {
            Self
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> MappedLocalTime<FixedOffset> {
            MappedLocalTime::None
        }
        fn offset_from_local_datetime(&self, _: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
            MappedLocalTime::None
        }
        fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
            Self::offset(&utc.and_time(NaiveTime::MIN))
        }
        fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
            Self::offset(utc)
        }
    }

    /// Fixed local fields follow the selected environment's date and clock presentation.
    #[test]
    fn local_date_time_uses_locale_observations() {
        let now = NaiveDate::from_ymd_opt(2026, 10, 27)
            .unwrap()
            .and_hms_opt(9, 25, 7)
            .unwrap();
        assert_eq!(
            format_local_date_time(now, &|key| (key == "LANG").then(|| "fr_CA.UTF-8".into()))
                .unwrap(),
            "2026-10-27 09 h 25 min 07 s"
        );
    }

    /// Fixed local calendar fields retain midnight, noon and unpadded month/day semantics.
    #[test]
    fn local_date_time_formats_fixed_clock_fields() {
        for (month, day, hour, expected) in [
            (1, 2, 0, "1/2/2026, 12:05:07 AM"),
            (1, 2, 12, "1/2/2026, 12:05:07 PM"),
            (10, 27, 9, "10/27/2026, 9:05:07 AM"),
            (12, 31, 23, "12/31/2026, 11:05:07 PM"),
        ] {
            let now = NaiveDate::from_ymd_opt(2026, month, day)
                .unwrap()
                .and_hms_opt(hour, 5, 7)
                .unwrap();
            assert_eq!(format_local_date_time(now, &|_| None).unwrap(), expected);
        }
    }

    /// Fixed local clock fields keep midnight, noon and the unpadded hour.
    #[test]
    fn local_time_formats_fixed_clock_fields() {
        for (hour, expected) in [
            (0, "12:05:07 AM"),
            (9, "9:05:07 AM"),
            (12, "12:05:07 PM"),
            (23, "11:05:07 PM"),
        ] {
            let at = NaiveDate::from_ymd_opt(2026, 10, 27)
                .unwrap()
                .and_hms_opt(hour, 5, 7)
                .unwrap();
            assert_eq!(format_local_time(at), expected);
        }
    }

    /// An instant outside the calendar's range reads as an invalid date.
    #[test]
    fn local_time_string_reports_instants_outside_the_calendar() {
        assert_eq!(
            crate::helpers::call("time.local_time_string", &format!("[{}]", i64::MAX)),
            Ok(r#""Invalid Date""#.to_owned())
        );
    }

    /// Instants count milliseconds: 1,000 advance the time one second, and fewer change
    /// nothing.
    #[test]
    fn local_time_string_counts_milliseconds() {
        let time = |millis: i64| {
            let answer = crate::helpers::call("time.local_time_string", &format!("[{millis}]"));
            let text: String = serde_json::from_str(&answer.unwrap()).unwrap();
            NaiveTime::parse_from_str(&text, "%I:%M:%S %p").unwrap()
        };
        assert_eq!(time(999), time(0));
        assert_eq!(time(1_000) - time(0), TimeDelta::seconds(1));
    }

    /// The epoch converts once through the supplied zone, on either side of UTC, and noon
    /// keeps its PM marker.
    #[test]
    fn local_time_converts_epoch_zero_through_the_zone() {
        let east = FixedOffset::east_opt(19_800).unwrap();
        let west = FixedOffset::west_opt(18_000).unwrap();
        assert_eq!(local_time_in(0, &Utc).as_deref(), Some("12:00:00 AM"));
        assert_eq!(local_time_in(0, &east).as_deref(), Some("5:30:00 AM"));
        assert_eq!(local_time_in(0, &west).as_deref(), Some("7:00:00 PM"));
        assert_eq!(
            local_time_in(43_200_000, &Utc).as_deref(),
            Some("12:00:00 PM")
        );
    }

    /// Each instant takes the offset in force at that instant across a daylight-saving start.
    #[test]
    fn local_time_uses_the_offset_at_the_instant() {
        let before = local_time_in(SPRING_FORWARD - 1_000, &Spring);
        assert_eq!(before.as_deref(), Some("1:59:59 AM"));
        assert_eq!(
            local_time_in(SPRING_FORWARD, &Spring).as_deref(),
            Some("3:00:00 AM")
        );
    }

    /// Instants beyond the calendar's range in either direction have no local time.
    #[test]
    fn local_time_has_none_outside_the_calendar() {
        assert_eq!(local_time_in(i64::MAX, &Utc), None);
        assert_eq!(local_time_in(i64::MIN, &Utc), None);
    }

    /// Operands that are not one integer instant are errors, never the current time.
    #[test]
    fn local_time_string_rejects_malformed_instants() {
        for data in ["[1.5]", r#"["0"]"#, "[]", "[0,1]", "null"] {
            assert!(
                crate::helpers::call("time.local_time_string", data).is_err(),
                "{data}"
            );
        }
    }
}
