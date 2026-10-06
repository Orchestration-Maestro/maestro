pub(super) fn whitespace(c: char) -> bool {
    matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

pub(super) fn trim(s: &str) -> &str {
    s.trim_matches(whitespace)
}

pub(super) fn command(body: &str) -> Option<&'static str> {
    let lower = body.to_ascii_lowercase();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    for (command, capability) in [("lgtmi", "issue"), ("lgtm", "pr")] {
        if lower.match_indices(command).any(|(i, _)| {
            (i == 0 || !word(lower.as_bytes()[i - 1]))
                && (i + command.len() == lower.len() || !word(lower.as_bytes()[i + command.len()]))
        }) {
            return Some(capability);
        }
    }
    None
}

pub(super) fn weekday(value: &str) -> Option<u32> {
    use chrono::{Datelike, Duration, NaiveDate};
    let (date, time) = value
        .split_once(['T', 't', ' '])
        .map_or((value, None), |(d, t)| (d, Some(t)));
    let year_len = if date.starts_with(['+', '-']) { 7 } else { 4 };
    let year_text = date.get(..year_len)?;
    if !year_text
        .trim_start_matches(['+', '-'])
        .bytes()
        .all(|b| b.is_ascii_digit())
        || year_text == "-000000"
    {
        return None;
    }
    let year = year_text.parse::<i32>().ok()?;
    let rest = date.get(year_len..)?;
    let (month, day) = match rest.len() {
        0 => (1, 1),
        3 if rest.starts_with('-') => (rest[1..].parse::<u32>().ok()?, 1),
        6 if rest.starts_with('-') && rest.as_bytes()[3] == b'-' => (
            rest[1..3].parse::<u32>().ok()?,
            rest[4..].parse::<u32>().ok()?,
        ),
        _ => return None,
    };
    if !(1..=31).contains(&day) {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year, month, 1)?
        .checked_add_signed(Duration::days(i64::from(day) - 1))?;
    let Some(time) = time else {
        return Some(date.weekday().num_days_from_sunday());
    };
    let (clock, offset) = if let Some(clock) = time.strip_suffix(['Z', 'z']) {
        (clock, 0)
    } else if let Some(index) = time.find(['+', '-']) {
        let zone = time.get(index..)?;
        let minute_start = match zone.len() {
            5 => 3,
            6 if zone.as_bytes()[3] == b':' => 4,
            _ => return None,
        };
        let hour = zone.get(1..3)?.parse::<i64>().ok()?;
        let minute = zone.get(minute_start..)?.parse::<i64>().ok()?;
        if hour > 23 || minute > 59 {
            return None;
        }
        let sign = if zone.starts_with('-') { -1 } else { 1 };
        (time.get(..index)?, sign * (hour * 60 + minute))
    } else {
        // Repository runners use UTC for timestamps without an explicit offset.
        (time, 0)
    };
    let (clock, fraction) = clock
        .split_once('.')
        .map_or((clock, None), |(c, f)| (c, Some(f)));
    if !matches!(clock.len(), 5 | 8)
        || clock.as_bytes()[2] != b':'
        || (clock.len() == 8 && clock.as_bytes()[5] != b':')
        || !clock
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 2 || i == 5 || b.is_ascii_digit())
    {
        return None;
    }
    let hour = clock[..2].parse::<i64>().ok()?;
    let minute = clock[3..5].parse::<i64>().ok()?;
    let second = if clock.len() == 8 {
        clock[6..].parse::<i64>().ok()?
    } else {
        0
    };
    if hour > 24
        || minute > 59
        || second > 59
        || fraction.is_some_and(|f| {
            clock.len() != 8 || f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit())
        })
        || (hour == 24
            && (minute != 0
                || second != 0
                || fraction.is_some_and(|f| f.bytes().any(|b| b != b'0'))))
    {
        return None;
    }
    let datetime = date
        .and_hms_opt(0, 0, 0)?
        .checked_add_signed(Duration::seconds(
            hour * 3600 + minute * 60 + second - offset * 60,
        ))?;
    Some(datetime.weekday().num_days_from_sunday())
}
