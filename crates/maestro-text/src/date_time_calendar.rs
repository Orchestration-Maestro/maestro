//! Gregorian wall-date validation and default-calendar conversion.
use std::io;

/// Persian leap corrections from ICU persncal.cpp, SHA-256
/// 31e7e93bb7dc1e4436c58ffd44fd1ed2686ceb79228b8d989f6573ec15ed5f45.
/// Source: <https://github.com/unicode-org/icu/blob/main/icu4c/source/i18n/persncal.cpp>.
/// The Unicode licence is retained beside the date-time data.
const CORRECTIONS: &[i64] = &[
    1502, 1601, 1634, 1667, 1700, 1733, 1766, 1799, 1832, 1865, 1898, 1931, 1964, 1997, 2030, 2059,
    2063, 2096, 2129, 2158, 2162, 2191, 2195, 2224, 2228, 2257, 2261, 2290, 2294, 2323, 2327, 2356,
    2360, 2389, 2393, 2422, 2426, 2455, 2459, 2488, 2492, 2521, 2525, 2554, 2558, 2587, 2591, 2620,
    2624, 2653, 2657, 2686, 2690, 2719, 2723, 2748, 2752, 2756, 2781, 2785, 2789, 2818, 2822, 2847,
    2851, 2855, 2880, 2884, 2888, 2913, 2917, 2921, 2946, 2950, 2954, 2979, 2983, 2987,
];

/// Checks Gregorian fields and returns the civil Julian day number.
fn julian_day([year, month, day]: [i32; 3]) -> io::Result<i64> {
    let leap = year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0);
    let length = match month {
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    if !(1..=length).contains(&day) {
        return Err(io::Error::other("invalid Gregorian wall date"));
    }
    let a = (14 - i64::from(month)) / 12;
    let y = i64::from(year) + 4800 - a;
    let m = i64::from(month) + 12 * a - 3;
    Ok(
        i64::from(day) + (153 * m + 2) / 5 + 365 * y + y.div_euclid(4) - y.div_euclid(100)
            + y.div_euclid(400)
            - 32045,
    )
}

/// Computes the Persian year start relative to its epoch.
fn persian_start(year: i64) -> i64 {
    365 * (year - 1) + (8 * year + 21).div_euclid(33)
        - i64::from(CORRECTIONS.binary_search(&(year - 1)).is_ok())
}

/// Converts validated Gregorian fields to the recipe's default calendar.
pub(super) fn convert(date: [i32; 3], calendar: &str) -> io::Result<[i64; 3]> {
    let julian = julian_day(date)?;
    let [year, month, day] = date.map(i64::from);
    match calendar {
        "gregory" => Ok([if year <= 0 { 1 - year } else { year }, month, day]),
        "buddhist" => Ok([year + 543, month, day]),
        "persian" => {
            let days = julian - 1_948_320;
            let mut year = (33 * days + 3).div_euclid(12_053) + 1;
            let mut ordinal = days - persian_start(year);
            if ordinal == 365 && CORRECTIONS.binary_search(&year).is_ok() {
                year += 1;
                ordinal = 0;
            }
            let month = if ordinal < 216 {
                ordinal / 31
            } else {
                (ordinal - 6) / 30
            };
            let start = if month < 6 {
                month * 31
            } else {
                month * 30 + 6
            };
            Ok([year, month + 1, ordinal - start + 1])
        }
        _ => Err(io::Error::other("unsupported date-time calendar")),
    }
}
