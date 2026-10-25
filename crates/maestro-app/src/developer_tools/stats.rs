//! Recorded assistant usage totalled over local calendar days.

use super::calendar::{days_after, days_before, local_midnight};
use super::formatting::{cost as amount, count, pad_end, pad_start};
use super::jsonl::kept_lines;
use super::{is_journal, resolve_path, session_base};
use crate::utils::numbers::decimal_integer_prefix;
use chrono::{Local, NaiveDate};
use maestro_session::SessionDir;
use maestro_text::utf16_order;
use num_traits::ToPrimitive;
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::io::{self, Write};
use std::process::ExitCode;

/// The help text, printed before any filesystem access.
const HELP: &str = "Usage: scripts/stats.ts [options]

Options:
  -n, --days <days>         Number of local calendar days to include (default: 7)
  -d, --dir, --cwd <path>   Project cwd to inspect (default: current cwd)
  --sessions-base <path>    Sessions base directory (default: ~/.maestro/agent/sessions)
  -h, --help                Show this help
";
/// Width of the report's double and single rules.
const RULE: usize = 160;

/// Reported token counts, costs and the files that produced them.
#[derive(Default)]
struct Totals {
    /// Reported input tokens.
    input: f64,
    /// Reported output tokens.
    output: f64,
    /// Reported cache-read tokens.
    cache_read: f64,
    /// Reported cache-write tokens.
    cache_write: f64,
    /// Reported total tokens.
    total_tokens: f64,
    /// Reported total cost.
    cost_total: f64,
    /// Number of assistant messages counted.
    assistant_messages: usize,
    /// Session files that contributed a message.
    sessions: HashSet<String>,
}

/// One local day's totals and its per-provider totals.
#[derive(Default)]
struct DayStats {
    /// The day's own totals.
    totals: Totals,
    /// Totals of each provider seen on that day.
    providers: BTreeMap<String, Totals>,
}

/// The selected window and session root for one report.
struct Options {
    /// Number of local calendar days in the window.
    days: u32,
    /// The resolved project working directory.
    cwd: String,
    /// The resolved session root.
    sessions_base: String,
}

/// Write the recorded-usage report for the requested window.
///
/// # Errors
/// Returns the native directory, read or write failure unchanged, and rejects a
/// recorded null entry the way the report's arithmetic does.
pub fn main(
    args: &[String],
    now: &dyn Fn() -> chrono::DateTime<Local>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let Some(options) = parse_options(args, stdout)? else {
        return Ok(ExitCode::SUCCESS);
    };
    let sessions = maestro_session::sessions_dir(&options.sessions_base, &options.cwd);
    if !std::path::Path::new(sessions.as_str()).exists() {
        writeln!(
            stderr,
            "Sessions directory not found: {}",
            sessions.as_str()
        )?;
        return Ok(ExitCode::from(1));
    }
    let today = now().date_naive();
    let start_date = days_before(today, options.days.saturating_sub(1));
    let window = Window {
        start: local_midnight(start_date),
        end: local_midnight(days_after(today, 1)),
    };
    let (days, grand) = collect(&sessions, &window)?;
    Report {
        options: &options,
        sessions: &sessions,
        start: start_date,
        today,
        days: &days,
        grand: &grand,
        stdout,
    }
    .write()
}

/// The reported local window, as epoch milliseconds.
struct Window {
    /// The first included instant.
    start: i64,
    /// The first excluded instant.
    end: i64,
}

/// Read the arguments, printing help when it is requested.
///
/// # Errors
/// Returns the native write failure of the help text and rejects an unusable day count.
fn parse_options(args: &[String], stdout: &mut dyn Write) -> io::Result<Option<Options>> {
    let mut days = 7.0;
    let mut cwd = None;
    let mut sessions_base = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        let value = args.get(index + 1).filter(|value| !value.is_empty());
        match arg.as_str() {
            "--days" | "-n" if value.is_some() => {
                days = decimal_integer_prefix(&args[index + 1]);
                index += 1;
            }
            "--cwd" | "--dir" | "-d" if value.is_some() => {
                cwd = Some(resolve_path(&args[index + 1])?);
                index += 1;
            }
            "--sessions-base" if value.is_some() => {
                sessions_base = Some(resolve_path(&args[index + 1])?);
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
    let Some(count) = (days.is_finite() && days.fract() == 0.0 && days > 0.0)
        .then(|| days.to_u32())
        .flatten()
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--days must be a positive integer",
        ));
    };
    let cwd = resolve_path(&cwd.unwrap_or_else(|| ".".to_owned()))?;
    let sessions_base = match sessions_base {
        Some(base) => base,
        None => session_base()?,
    };
    Ok(Some(Options {
        days: count,
        cwd,
        sessions_base,
    }))
}

/// Read every journal that contributes to the report.
///
/// # Errors
/// Returns the native directory or read failure unchanged, and rejects a null entry.
fn collect(
    sessions: &SessionDir,
    window: &Window,
) -> io::Result<(BTreeMap<String, DayStats>, Totals)> {
    let mut days: BTreeMap<String, DayStats> = BTreeMap::new();
    let mut grand = Totals::default();
    for entry in maestro_listing::read_dir(sessions.as_str())? {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_journal(&name) {
            continue;
        }
        let bytes = std::fs::read(entry.path())?;
        let content = String::from_utf8_lossy(&bytes);
        for line in kept_lines(&content) {
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            accumulate(&mut days, &mut grand, &value, window, &name)?;
        }
    }
    Ok((days, grand))
}

/// Add one recorded entry's usage to its day, provider and grand totals.
fn accumulate(
    days: &mut BTreeMap<String, DayStats>,
    grand: &mut Totals,
    entry: &Value,
    window: &Window,
    file: &str,
) -> io::Result<()> {
    let Some(entry) = entry.as_object() else {
        if entry.is_null() {
            return Err(io::Error::other("a recorded entry is null"));
        }
        return Ok(());
    };
    if entry.get("type").and_then(Value::as_str) != Some("message") {
        return Ok(());
    }
    let Some(message) = entry.get("message").and_then(Value::as_object) else {
        return Ok(());
    };
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return Ok(());
    }
    let Some(usage) = message
        .get("usage")
        .filter(|usage| maestro_json::is_truthy(usage))
    else {
        return Ok(());
    };
    let instant = reported_instant(message, entry.get("timestamp"));
    if instant.is_some_and(|millis| millis < window.start || millis >= window.end) {
        return Ok(());
    }
    let day = instant
        .and_then(super::calendar::Day::local)
        .map_or_else(|| "NaN-NaN-NaN".to_owned(), |day| day.to_string());
    let provider = message
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let day_stats = days.entry(day).or_default();
    let provider_totals = day_stats.providers.entry(provider).or_default();
    add_usage(&mut day_stats.totals, usage, file);
    add_usage(provider_totals, usage, file);
    add_usage(grand, usage, file);
    Ok(())
}

/// The reported instant, preferring a present message timestamp over the entry's.
fn reported_instant(
    message: &serde_json::Map<String, Value>,
    entry: Option<&Value>,
) -> Option<i64> {
    match message.get("timestamp") {
        Some(value) => instant(value),
        None => instant(entry.unwrap_or(&Value::from(0))),
    }
}

/// Read a recorded instant as epoch milliseconds.
fn instant(value: &Value) -> Option<i64> {
    match value {
        Value::Null => Some(0),
        Value::Number(number) => number.as_f64().and_then(|millis| millis.to_i64()),
        Value::String(text) => maestro_session::messages::parse_timestamp_millis(text),
        _ => None,
    }
}

/// One numeric usage member, absent and non-numeric members being zero.
fn member(value: &Value, name: &str) -> f64 {
    value
        .as_object()
        .and_then(|object| object.get(name))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

/// Add one recorded usage to a totals row.
fn add_usage(totals: &mut Totals, usage: &Value, file: &str) {
    let cost = usage
        .as_object()
        .and_then(|usage| usage.get("cost"))
        .unwrap_or(&Value::Null);
    totals.input += member(usage, "input");
    totals.output += member(usage, "output");
    totals.cache_read += member(usage, "cacheRead");
    totals.cache_write += member(usage, "cacheWrite");
    totals.total_tokens += member(usage, "totalTokens");
    totals.cost_total += member(cost, "total");
    totals.assistant_messages += 1;
    totals.sessions.insert(file.to_owned());
}

/// The rows one report writes, gathered so each writer stays small.
struct Report<'a> {
    /// The selected window and session root.
    options: &'a Options,
    /// The session directory the rows came from.
    sessions: &'a SessionDir,
    /// The first day of the window.
    start: NaiveDate,
    /// The day the report was written.
    today: NaiveDate,
    /// The day rows.
    days: &'a BTreeMap<String, DayStats>,
    /// The grand total row.
    grand: &'a Totals,
    /// The report's standard output.
    stdout: &'a mut dyn Write,
}

impl Report<'_> {
    /// Write the whole report.
    ///
    /// # Errors
    /// Returns the native write failure unchanged.
    fn write(self) -> io::Result<ExitCode> {
        writeln!(self.stdout, "Usage for {}", self.options.cwd)?;
        writeln!(self.stdout, "Sessions: {}", self.sessions.as_str())?;
        writeln!(
            self.stdout,
            "Period: {} through {} ({} local days)",
            day_text(self.start),
            day_text(self.today),
            self.options.days
        )?;
        writeln!(self.stdout, "{}", "=".repeat(RULE))?;
        for (day, stats) in self.days {
            write_totals(self.stdout, day, &stats.totals)?;
            let mut providers: Vec<&String> = stats.providers.keys().collect();
            providers.sort_by(|left, right| utf16_order(left, right));
            for provider in providers {
                let label = format!("  {provider}");
                write_totals(self.stdout, &label, &stats.providers[provider])?;
            }
        }
        writeln!(self.stdout, "{}", "-".repeat(RULE))?;
        write_totals(self.stdout, "TOTAL", self.grand)?;
        Ok(ExitCode::SUCCESS)
    }
}

/// Write a date the way a report row labels it.
fn day_text(date: NaiveDate) -> String {
    super::calendar::Day::text(date)
}

/// Write one totals row.
fn write_totals(stdout: &mut dyn Write, label: &str, totals: &Totals) -> io::Result<()> {
    writeln!(
        stdout,
        "{} messages: {}  sessions: {}  input: {}  output: {}  cache read: {}  cache write: {}  total: {}  cost: {}",
        pad_end(label, 16),
        pad_start(&totals.assistant_messages.to_string(), 5),
        pad_start(&totals.sessions.len().to_string(), 3),
        pad_start(&count(totals.input)?, 12),
        pad_start(&count(totals.output)?, 10),
        pad_start(&count(totals.cache_read)?, 13),
        pad_start(&count(totals.cache_write)?, 10),
        pad_start(&count(totals.total_tokens)?, 13),
        pad_start(&format!("${}", amount(totals.cost_total)), 10)
    )
}
