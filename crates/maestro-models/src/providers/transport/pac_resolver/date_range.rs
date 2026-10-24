//! Documented calendar profiles with inclusive, wrapping range endpoints.

use boa_engine::{Context, JsResult, JsValue};
use chrono::NaiveDate;

/// A calendar operand keeps month names distinct from numeric days and years.
#[derive(Clone, Copy)]
enum Part {
    /// Numeric day or full year, distinguished by its profile.
    Number(i32),
    /// One-based month from an exact uppercase name.
    Month(i32),
}

/// Project a single native instant into the requested local or GMT calendar.
pub(super) fn date_range(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let (args, gmt) = super::util::timezone(args);
    let now = super::util::calendar(gmt, context)?;
    let values: Option<Vec<_>> = args
        .iter()
        .map(|value| {
            value
                .to_string(context)
                .map(|text| part(&text.to_std_string_escaped()))
        })
        .collect::<JsResult<_>>()?;
    Ok(values
        .is_some_and(|values| matches(&values, [now[0], now[1], now[2]]))
        .into())
}

/// Read month names before the native decimal integer prefix.
fn part(text: &str) -> Option<Part> {
    [
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ]
    .iter()
    .zip(1..)
    .find_map(|(name, index)| (*name == text).then_some(Part::Month(index)))
    .or_else(|| super::util::decimal_prefix(text).map(Part::Number))
}

/// Choose the documented profile without conflating dates with ranges.
fn matches(values: &[Part], now: [i32; 3]) -> bool {
    use Part::{Month as M, Number as N};
    let [year, month, day] = now;
    match *values {
        [N(value)] if value < 32 => day == value,
        [N(value)] => year == value,
        [M(value)] => month == value,
        [N(start), N(end)] if start < 32 && end < 32 => wrapped(start, day, end),
        [N(start), N(end)] if start >= 32 && end >= 32 => wrapped(start, year, end),
        [M(start), M(end)] => wrapped(start, month, end),
        [N(d), M(m)] => (day, month) == (d, m),
        [N(d), M(m), N(y)] => (day, month, year) == (d, m, y),
        [N(d1), M(m1), N(d2), M(m2)] => dates([year, m1, d1], now, [year, m2, d2]),
        [M(m1), N(y1), M(m2), N(y2)] => wrapped((y1, m1), (year, month), (y2, m2)),
        [N(d1), M(m1), N(y1), N(d2), M(m2), N(y2)] => dates([y1, m1, d1], now, [y2, m2, d2]),
        _ => false,
    }
}

/// Retain both endpoints even when a range crosses its calendar boundary.
fn wrapped<T: Ord + Copy>(start: T, now: T, end: T) -> bool {
    if start <= end {
        start <= now && now <= end
    } else {
        now >= start || now <= end
    }
}

/// Validate full calendar dates, including leap days, before ordering them.
fn dates(start: [i32; 3], now: [i32; 3], end: [i32; 3]) -> bool {
    let date = |[year, month, day]: [i32; 3]| {
        NaiveDate::from_ymd_opt(year, month.try_into().ok()?, day.try_into().ok()?)
    };
    match (date(start), date(now), date(end)) {
        (Some(start), Some(now), Some(end)) => wrapped(start, now, end),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::Part::{Month as M, Number as N};

    #[test]
    fn calendar_profiles_keep_single_dates_ranges_and_leap_days() {
        let now = [2024, 2, 29];
        for args in [
            vec![N(29)],
            vec![M(2)],
            vec![N(2024)],
            vec![N(29), M(2)],
            vec![N(29), M(2), N(2024)],
            vec![N(1), N(29)],
            vec![M(1), M(3)],
            vec![N(2023), N(2025)],
            vec![N(1), M(2), N(29), M(2)],
            vec![M(2), N(2024), M(3), N(2024)],
            vec![N(29), M(2), N(2024), N(1), M(3), N(2024)],
        ] {
            assert!(super::matches(&args, now));
        }
        assert!(!super::matches(
            &[N(29), M(2), N(2023), N(1), M(3), N(2023)],
            now
        ));
        assert!(!super::matches(&[N(28)], now));
        assert!(!super::matches(&[M(3)], now));
        assert!(!super::matches(&[N(2023)], now));
        assert!(super::matches(&[M(12), M(2)], now));
        assert!(super::matches(&[N(31), N(2)], [2024, 3, 1]));
        assert!(super::matches(&[N(1), M(12), N(1), M(3)], [2025, 1, 1]));
        assert!(!super::matches(&[N(1), M(12), N(1), M(3)], [2025, 7, 1]));
        assert!(super::matches(&[M(12), N(2023), M(2), N(2024)], now));
    }
}
