//! Recorded message costs totalled per UTC day and provider.

use super::calendar::{Day, local_midnight};
use super::formatting::{cost as amount, pad_end, pad_start};
use super::jsonl::trimmed_lines;
use super::{cost_session_base, is_journal};
use crate::utils::numbers::decimal_integer_prefix;
use chrono::{Local, NaiveDate, TimeDelta};
use maestro_session::SessionDir;
use maestro_text::utf16_order;
use num_traits::ToPrimitive;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::process::ExitCode;

/// The help text, printed before any filesystem access.
const HELP: &str = "Usage: cost.ts -d <path> -n <days>
  -d, --dir <path>   Directory path (required)
  -n, --days <num>   Number of days to track (required)
  -h, --help         Show this help
";
/// The diagnostic for a missing directory or day count.
const REQUIRED: &str = "Error: both --dir and --days are required\nRun with --help for usage\n";
/// Column width of a provider or total label.
const NAME_COLUMNS: usize = 15;
/// Column width of a cost amount.
const AMOUNT_COLUMNS: usize = 8;
/// Width of the report's double rule.
const SEPARATOR: usize = 80;
/// Width of the report's single rule.
const RULE: usize = 40;

/// One day's or one provider's accumulated costs.
#[derive(Clone, Copy, Default)]
struct DayCost {
    /// Reported total cost.
    total: f64,
    /// Reported input cost.
    input: f64,
    /// Reported output cost.
    output: f64,
    /// Reported cache-read cost.
    cache_read: f64,
    /// Reported cache-write cost.
    cache_write: f64,
    /// Number of recorded requests.
    requests: usize,
}

/// Write the recorded-cost report for the requested directory and window.
///
/// # Errors
/// Returns the native directory, read or write failure unchanged.
pub fn main(
    args: &[String],
    now: &dyn Fn() -> chrono::DateTime<Local>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(directory) = parse_options(args, stdout)? else {
        return Ok(ExitCode::SUCCESS);
    };
    let Some(days) = directory.days.filter(|days| *days != 0.0 && !days.is_nan()) else {
        stderr.write_all(REQUIRED.as_bytes())?;
        return Ok(ExitCode::from(1));
    };
    let Some(project) = directory.directory.filter(|dir| !dir.is_empty()) else {
        stderr.write_all(REQUIRED.as_bytes())?;
        return Ok(ExitCode::from(1));
    };
    let sessions = maestro_session::sessions_dir(&cost_session_base()?, &project);
    if !std::path::Path::new(sessions.as_str()).exists() {
        writeln!(
            stderr,
            "Sessions directory not found: {}",
            sessions.as_str()
        )?;
        return Ok(ExitCode::from(1));
    }
    let today = now().date_naive();
    let cutoff = local_midnight(days_before(today, days));
    let report = collect(&sessions, cutoff)?;
    if report.is_empty() {
        writeln!(
            stdout,
            "No sessions found in the last {} days for: {project}",
            number(days)
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    write_report(&report, &project, days, cutoff, stdout)
}

/// The authored directory and day count, each kept only when supplied.
struct Options {
    /// The authored directory, absent when the flag had no value.
    directory: Option<String>,
    /// The parsed day count, not a number when the flag had no value.
    days: Option<f64>,
}

/// Read the arguments, printing help when it is requested.
///
/// # Errors
/// Returns the native write failure of the help text.
fn parse_options(args: &[String], stdout: &mut dyn Write) -> io::Result<Option<Options>> {
    let mut options = Options {
        directory: None,
        days: None,
    };
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--dir" | "-d" => {
                options.directory = args.get(index + 1).cloned();
                index += 1;
            }
            "--days" | "-n" => {
                options.days = Some(
                    args.get(index + 1)
                        .map_or(f64::NAN, |value| decimal_integer_prefix(value)),
                );
                index += 1;
            }
            "--help" | "-h" => {
                stdout.write_all(HELP.as_bytes())?;
                return Ok(None);
            }
            _ => {}
        }
        index += 1;
    }
    Ok(Some(options))
}

/// The date `days` before `today`, keeping `today` when the count is unusable.
fn days_before(today: NaiveDate, days: f64) -> NaiveDate {
    days.to_i64()
        .and_then(|days| TimeDelta::try_days(-days))
        .and_then(|delta| today.checked_add_signed(delta))
        .unwrap_or(today)
}

/// The report rows, kept in day and provider key order.
type Report = BTreeMap<String, BTreeMap<String, DayCost>>;

/// Read every journal in the session directory that the cutoff admits.
///
/// # Errors
/// Returns the native directory or read failure unchanged.
fn collect(sessions: &SessionDir, cutoff: i64) -> io::Result<Report> {
    let mut report = Report::new();
    for entry in maestro_listing::read_dir(sessions.as_str())? {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_journal(&name) || file_instant(&name).is_some_and(|date| date < cutoff) {
            continue;
        }
        let bytes = std::fs::read(entry.path())?;
        let content = String::from_utf8_lossy(&bytes);
        for line in trimmed_lines(&content) {
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            accumulate(&mut report, &value);
        }
    }
    Ok(report)
}

/// Add one recorded entry's cost to its day and provider row.
fn accumulate(report: &mut Report, entry: &Value) {
    let Some(entry) = entry.as_object() else {
        return;
    };
    if entry.get("type").and_then(Value::as_str) != Some("message") {
        return;
    }
    let Some(message) = entry.get("message").and_then(Value::as_object) else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let Some(usage) = message.get("usage").and_then(Value::as_object) else {
        return;
    };
    let Some(cost) = usage
        .get("cost")
        .filter(|cost| maestro_json::is_truthy(cost))
    else {
        return;
    };
    let Some(instant) = entry.get("timestamp").and_then(instant) else {
        return;
    };
    let Some(day) = Day::utc(instant) else {
        return;
    };
    let provider = label(message.get("provider"));
    let row = report
        .entry(day.to_string())
        .or_default()
        .entry(provider)
        .or_default();
    row.total += cost_of(cost, "total");
    row.input += cost_of(cost, "input");
    row.output += cost_of(cost, "output");
    row.cache_read += cost_of(cost, "cacheRead");
    row.cache_write += cost_of(cost, "cacheWrite");
    row.requests += 1;
}

/// The property key a provider value contributes when read as a map key.
fn label(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_owned(),
        Some(Value::Null) => "null".to_owned(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Number(number)) => ryu_js::Buffer::new()
            .format_finite(number.as_f64().unwrap_or_default())
            .to_owned(),
        Some(Value::Array(_) | Value::Object(_)) => "[object Object]".to_owned(),
    }
}

/// Read `field` from a recorded amount, absent and non-numeric amounts being zero.
fn cost_of(value: &Value, field: &str) -> f64 {
    value
        .as_object()
        .and_then(|object| object.get(field))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

/// Read a recorded instant, as milliseconds, from a text or number value.
fn instant(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_f64().and_then(|millis| millis.to_i64()),
        Value::String(text) => maestro_session::messages::parse_timestamp_millis(text),
        _ => None,
    }
}

/// Read the session file's own instant from its `<stamp>_<id>.jsonl` name.
fn file_instant(name: &str) -> Option<i64> {
    let stamp = name.split('_').next().unwrap_or(name);
    instant(&Value::String(dashed_stamp(stamp)))
}

/// Rewrite the filename's `T08-25-07-381Z` clock into an ISO timestamp.
fn dashed_stamp(stamp: &str) -> String {
    let Some((date, rest)) = stamp.split_once('T') else {
        return stamp.to_owned();
    };
    let bytes = rest.as_bytes();
    let digits = [0, 1, 3, 4, 6, 7, 9, 10, 11];
    let shaped = bytes.len() >= 13
        && bytes[2] == b'-'
        && bytes[5] == b'-'
        && bytes[8] == b'-'
        && bytes[12] == b'Z'
        && digits.iter().all(|index| bytes[*index].is_ascii_digit());
    if !shaped {
        return stamp.to_owned();
    }
    format!(
        "{date}T{}:{}:{}.{}Z",
        &rest[0..2],
        &rest[3..5],
        &rest[6..8],
        &rest[9..12]
    )
}

/// Spell a count the way the report's period line does.
fn number(value: f64) -> String {
    ryu_js::Buffer::new().format_finite(value).to_owned()
}

/// Write the day, provider and grand-total sections.
///
/// # Errors
/// Returns the native write failure unchanged.
fn write_report(
    report: &Report,
    project: &str,
    days: f64,
    cutoff: i64,
    stdout: &mut dyn Write,
) -> io::Result<ExitCode> {
    let since = Day::utc(cutoff).map_or_else(|| "NaN-NaN-NaN".to_owned(), |day| day.to_string());
    writeln!(stdout, "\nCost breakdown for: {project}")?;
    writeln!(stdout, "Period: last {} days (since {since})", number(days))?;
    writeln!(stdout, "{}", "=".repeat(SEPARATOR))?;
    let mut totals: BTreeMap<String, DayCost> = BTreeMap::new();
    let mut grand = 0.0;
    for (day, providers) in report {
        writeln!(stdout, "\n{day}")?;
        writeln!(stdout, "{}", "-".repeat(RULE))?;
        let mut day_total = 0.0;
        for provider in sorted(providers) {
            let row = &providers[provider];
            day_total += row.total;
            add(totals.entry(provider.to_owned()).or_default(), row);
            write_row(stdout, provider, row)?;
        }
        writeln!(
            stdout,
            "  {} ${}",
            pad_end("Day total:", NAME_COLUMNS),
            pad_start(&amount(day_total), AMOUNT_COLUMNS)
        )?;
        grand += day_total;
    }
    writeln!(stdout, "\n{}", "=".repeat(SEPARATOR))?;
    writeln!(stdout, "TOTALS BY PROVIDER")?;
    writeln!(stdout, "{}", "-".repeat(RULE))?;
    for provider in sorted(&totals) {
        write_row(stdout, provider, &totals[provider])?;
    }
    writeln!(stdout, "{}", "-".repeat(RULE))?;
    writeln!(
        stdout,
        "  {} ${}",
        pad_end("GRAND TOTAL:", NAME_COLUMNS),
        pad_start(&amount(grand), AMOUNT_COLUMNS)
    )?;
    writeln!(stdout)?;
    Ok(ExitCode::SUCCESS)
}

/// The row labels in UTF-16 order.
fn sorted(rows: &BTreeMap<String, DayCost>) -> Vec<&str> {
    let mut keys: Vec<&str> = rows.keys().map(String::as_str).collect();
    keys.sort_by(|left, right| utf16_order(left, right));
    keys
}

/// Fold one row's amounts into a total.
fn add(total: &mut DayCost, row: &DayCost) {
    total.total += row.total;
    total.input += row.input;
    total.output += row.output;
    total.cache_read += row.cache_read;
    total.cache_write += row.cache_write;
    total.requests += row.requests;
}

/// Write one provider or day row.
fn write_row(stdout: &mut dyn Write, provider: &str, row: &DayCost) -> io::Result<()> {
    writeln!(
        stdout,
        "  {} ${}  ({} reqs, in: ${}, out: ${}, cache: ${})",
        pad_end(provider, NAME_COLUMNS),
        pad_start(&amount(row.total), AMOUNT_COLUMNS),
        row.requests,
        amount(row.input),
        amount(row.output),
        amount(row.cache_read + row.cache_write)
    )
}
