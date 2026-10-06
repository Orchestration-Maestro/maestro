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
        .split_once('T')
        .map_or((value, None), |(d, t)| (d, Some(t)));
    let parts: Vec<_> = date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year = parts[0].parse::<i32>().ok()?;
    let month = parts[1].parse::<u32>().ok()?;
    let day = parts[2].parse::<u32>().ok()?;
    if !(1..=31).contains(&day) {
        return None;
    }
    let date = NaiveDate::from_ymd_opt(year, month, 1)?
        .checked_add_signed(Duration::days(i64::from(day) - 1))?;
    let Some(time) = time else {
        return Some(date.weekday().num_days_from_sunday());
    };
    let mut time = time.to_owned();
    let hour = time.get(..2)?.parse::<u32>().ok()?;
    let minute = time.get(3..5)?.parse::<u32>().ok()?;
    let second = time.get(6..8)?.parse::<u32>().ok()?;
    if hour > 24 || minute > 59 || second > 59 {
        return None;
    }
    let next_day = hour == 24;
    if next_day {
        if minute != 0 || second != 0 {
            return None;
        }
        let fraction = time.get(8..).unwrap_or("");
        if fraction.starts_with('.')
            && fraction[1..]
                .chars()
                .take_while(char::is_ascii_digit)
                .any(|c| c != '0')
        {
            return None;
        }
        time.replace_range(..2, "00");
    }
    if !time.ends_with('Z') && !time.contains('+') && !time.contains('-') {
        time.push('Z');
    }
    let normalized = format!("{date}T{time}");
    let date = chrono::DateTime::parse_from_rfc3339(&normalized)
        .ok()?
        .with_timezone(&chrono::Utc);
    let date = if next_day {
        date.checked_add_signed(Duration::days(1))?
    } else {
        date
    };
    Some(date.weekday().num_days_from_sunday())
}
