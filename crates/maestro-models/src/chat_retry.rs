//! Setup-only retry eligibility and uncapped controlled delays.
use crate::*;
use std::{collections::BTreeMap, time::Duration};
pub(crate) fn eligible(status: u16, headers: &BTreeMap<String, String>) -> bool {
    match headers.get("x-should-retry").map(String::as_str) {
        Some("true") => true,
        Some("false") => false,
        _ => matches!(status, 408 | 409 | 429 | 500..=599),
    }
}
pub(crate) fn delay(
    headers: &BTreeMap<String, String>,
    index: u32,
    transport: &dyn ChatTransport,
) -> Duration {
    if let Some(d) = headers
        .get("retry-after-ms")
        .and_then(|s| numeric(s, 0.001))
    {
        return d;
    }
    if let Some(d) = headers
        .get("retry-after")
        .and_then(|s| numeric(s, 1.0).or_else(|| date(s, transport.now_unix_millis())))
    {
        return d;
    }
    Duration::from_secs_f64(
        (500.0 * 2f64.powi(index.min(4) as i32)).min(8000.0) * (1.0 - transport.jitter()) / 1000.0,
    )
}
pub(crate) async fn setup(
    transport: &dyn ChatTransport,
    request: ChatHttpRequest,
    options: &ProviderOptions,
) -> Result<ChatHttpResponse, Failure> {
    let timeout = options.timeout_ms.unwrap_or(600000);
    let retries = options.max_retries.unwrap_or(2);
    for attempt in 0..=retries {
        if options.cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        let response = if timeout == 0 {
            Err(Failure::SetupTimeout)
        } else {
            transport
                .send(request.clone(), timeout, options.cancellation.clone())
                .await
        };
        if options.cancellation.is_cancelled() {
            return Err(Failure::Cancelled);
        }
        match response {
            Ok(response) if (200..300).contains(&response.status) => return Ok(response),
            Ok(mut response) => {
                if attempt < retries && eligible(response.status, &response.headers) {
                    let wait = delay(&response.headers, attempt, transport);
                    drop(response);
                    transport.wait(wait, options.cancellation.clone()).await?;
                    continue;
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.body.next().await? {
                    bytes.extend(chunk);
                }
                let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
                if bytes.is_empty() && matches!(response.status, 400 | 413) {
                    return Err(Failure::ContextOverflow);
                }
                return Err(crate::chat_failure::classify(Some(response.status), &value));
            }
            Err(failure)
                if attempt < retries
                    && matches!(failure, Failure::Transport | Failure::SetupTimeout) =>
            {
                transport
                    .wait(
                        delay(&BTreeMap::new(), attempt, transport),
                        options.cancellation.clone(),
                    )
                    .await?;
            }
            Err(failure) => return Err(failure),
        }
    }
    unreachable!()
}

fn numeric(s: &str, scale: f64) -> Option<Duration> {
    let s = crate::scalar::trim(s);
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return None;
    }
    let value = s.parse::<f64>().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(value * scale).ok()
}
fn date(s: &str, now: u64) -> Option<Duration> {
    let s = crate::scalar::trim(s);
    if !s.is_ascii() {
        return None;
    }
    let words = s.split(' ').collect::<Vec<_>>();
    let month = |s: &str| {
        [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ]
        .iter()
        .position(|m| *m == s)
        .map(|i| i as i64 + 1)
    };
    let parse = |s: &str| {
        if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
            s.parse::<i64>().ok()
        } else {
            None
        }
    };
    let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let long = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let mut obsolete = false;
    let (mut year, month, day, time, weekday) = if words.len() == 6
        && words[0].len() == 4
        && words[0].ends_with(',')
        && words[5] == "GMT"
    {
        if words[1].len() != 2 || words[3].len() != 4 {
            return None;
        }
        (
            parse(words[3])?,
            month(words[2])?,
            parse(words[1])?,
            words[4],
            weekdays.iter().position(|w| *w == &words[0][..3])?,
        )
    } else if words.len() == 4 && words[0].ends_with(',') && words[3] == "GMT" {
        let parts = words[1].split('-').collect::<Vec<_>>();
        if parts.len() != 3 || parts[0].len() != 2 || parts[2].len() != 2 {
            return None;
        }
        obsolete = true;
        let current_year = year_from_days((now / 86400000) as i64);
        let mut year = current_year / 100 * 100 + parse(parts[2])?;
        if year <= current_year - 50 {
            year += 100;
        }
        if year > current_year + 50 {
            year -= 100
        }
        (
            year,
            month(parts[1])?,
            parse(parts[0])?,
            words[2],
            long.iter()
                .position(|w| *w == words[0].strip_suffix(',').unwrap_or(""))?,
        )
    } else if (words.len() == 5 || words.len() == 6) && s.len() == 24 {
        let weekday = weekdays.iter().position(|w| *w == &s[..3])?;
        if &s[3..4] != " " || &s[7..8] != " " || &s[10..11] != " " || &s[19..20] != " " {
            return None;
        }
        (
            parse(&s[20..])?,
            month(&s[4..7])?,
            parse(s[8..10].trim_start_matches(' '))?,
            &s[11..19],
            weekday,
        )
    } else {
        return None;
    };
    if time.len() != 8 || &time[2..3] != ":" || &time[5..6] != ":" {
        return None;
    }
    let hour = parse(&time[..2])?;
    let minute = parse(&time[3..5])?;
    let second = parse(&time[6..])?;
    if hour > 23 || minute > 59 || second > 59 || year < 1601 || day < 1 {
        return None;
    }
    if obsolete {
        let now_days = (now / 86400000) as i64;
        let current_year = year_from_days(now_days);
        let mut current_month = 1;
        while current_month < 12 && days_from_civil(current_year, current_month + 1, 1) <= now_days
        {
            current_month += 1;
        }
        let current_day = now_days - days_from_civil(current_year, current_month, 1) + 1;
        let boundary = days_from_civil(current_year + 50, current_month, current_day) as i128
            * 86400000
            + (now % 86400000) as i128;
        let candidate = (days_from_civil(year, month, day) as i128 * 86400
            + hour as i128 * 3600
            + minute as i128 * 60
            + second as i128)
            * 1000;
        if candidate > boundary {
            year -= 100;
        }
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day > days[month as usize - 1] {
        return None;
    }
    let days = days_from_civil(year, month, day);
    if (days + 4).rem_euclid(7) as usize != weekday {
        return None;
    }
    let millis =
        (days as i128 * 86400 + hour as i128 * 3600 + minute as i128 * 60 + second as i128) * 1000;
    let delay = (millis - now as i128).max(0);
    Some(Duration::from_millis(u64::try_from(delay).ok()?))
}
fn days_from_civil(mut year: i64, month: i64, day: i64) -> i64 {
    year -= i64::from(month <= 2);
    let era = year.div_euclid(400);
    let y = year - era * 400;
    let m = month + if month > 2 { -3 } else { 9 };
    era * 146097 + y * 365 + y / 4 - y / 100 + (153 * m + 2) / 5 + day - 1 - 719468
}
fn year_from_days(days: i64) -> i64 {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let d = z - era * 146097;
    let y = (d - d / 1460 + d / 36524 - d / 146096) / 365;
    let doy = d - (365 * y + y / 4 - y / 100);
    let m = (5 * doy + 2) / 153;
    y + era * 400 + i64::from(m >= 10)
}
