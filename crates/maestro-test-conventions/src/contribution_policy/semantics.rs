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
    let (date, time) = value
        .split_once(['T', 't'])
        .map_or((value, None), |(d, t)| (d, Some(t)));
    let year_len = if date.starts_with(['+', '-']) { 7 } else { 4 };
    let year_text = date.get(..year_len)?;
    if !year_text
        .trim_start_matches(['+', '-'])
        .bytes()
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if year_text == "-000000" {
        return None;
    }
    let year = year_text.parse::<i32>().ok()?;
    let rest = date.get(year_len..)?;
    let (month, day) = match rest.len() {
        0 => (1, 1),
        3 if rest.starts_with('-') => (digits(&rest[1..], 2)? as u32, 1),
        6 if rest.starts_with('-') && rest.as_bytes()[3] == b'-' => (
            digits(&rest[1..3], 2)? as u32,
            digits(&rest[4..], 2)? as u32,
        ),
        _ => return None,
    };
    if !(1..=31).contains(&day) {
        return None;
    }
    if !(1..=12).contains(&month) {
        return None;
    }
    // MakeDay counts Gregorian days from the epoch, including normalized day overflow.
    let year = i64::from(year);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let days = 365 * (year - 1970) + (year - 1969).div_euclid(4) - (year - 1901).div_euclid(100)
        + (year - 1601).div_euclid(400)
        + month_days[(month - 1) as usize]
        + i64::from(leap && month > 2)
        + i64::from(day)
        - 1;
    let Some(time) = time else {
        return clipped_weekday(days * 86_400_000);
    };
    let (clock, offset) = if let Some(clock) = time.strip_suffix(['Z', 'z']) {
        (clock, 0)
    } else if let Some(index) = time.find(['+', '-']) {
        let zone = time.get(index..)?;
        let fields = zone.get(1..)?;
        let (hour_text, minute_text) = if let Some((hour, minute)) = fields.split_once(':') {
            (hour, minute)
        } else if fields.len() == 4 {
            (fields.get(..2)?, fields.get(2..)?)
        } else {
            return None;
        };
        let hour = digits(hour_text, 2)?;
        let minute = digits(minute_text, 2)?;
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
    let hour = digits(&clock[..2], 2)?;
    let minute = digits(&clock[3..5], 2)?;
    let second = if clock.len() == 8 {
        digits(&clock[6..], 2)?
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
    let millis = fraction.map_or(0, |f| {
        f.bytes()
            .take(3)
            .zip([100, 10, 1])
            .fold(0, |n, (b, scale)| n + i64::from(b - b'0') * scale)
    });
    // MakeDate combines the day and clock; TimeClip rejects values beyond either limit.
    clipped_weekday(
        days * 86_400_000 + (hour * 3600 + minute * 60 + second - offset * 60) * 1000 + millis,
    )
}

fn digits(value: &str, width: usize) -> Option<i64> {
    if value.len() != width || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn clipped_weekday(time: i64) -> Option<u32> {
    if time.abs() > 8_640_000_000_000_000 {
        return None;
    }
    Some((time.div_euclid(86_400_000) + 4).rem_euclid(7) as u32)
}
