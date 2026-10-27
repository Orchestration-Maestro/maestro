//! Host-local wall-clock presentation for authored input feedback.
use chrono::{Local, NaiveDateTime};

use super::{decode, encode};

/// Reads the host's current local time after accepting no operands.
pub(super) fn local_date_time_string(data: &str) -> Result<String, String> {
    let (): () = decode(data)?;
    encode(&format_local_date_time(
        Local::now().naive_local(),
        &|key| std::env::var(key).ok(),
    )?)
}

/// Formats local calendar fields with the en-US date and twelve-hour clock pattern.
fn format_local_date_time(
    now: NaiveDateTime,
    _environment: &dyn Fn(&str) -> Option<String>,
) -> Result<String, String> {
    let mut output = String::new();
    now.format("%-m/%-d/%Y, %-I:%M:%S %p")
        .write_to(&mut output)
        .map_err(|error| error.to_string())?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::format_local_date_time;

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
}
