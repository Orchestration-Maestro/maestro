//! Recorded tool-call statistics and the interactive report page.

mod template;

use super::jsonl::kept_lines;
use super::{resolve_path, session_base};
use indexmap::IndexMap;
use maestro_models::{tokens_for_chars, utf16_units};
use num_traits::ToPrimitive;
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

/// The help text, printed before any session access.
const HELP: &str = "Usage: scripts/tool-stats.ts [--sessions-dir <dir>] [--output <file.html>]\n";
/// The lower bound of each histogram bucket; the last bucket is open ended.
const BUCKETS: [u64; 11] = [0, 50, 100, 250, 500, 1000, 2000, 4000, 8000, 16000, 32000];
/// The number of bash command rows the report keeps.
const BASH_ROWS: usize = 50;

/// The counters the report keeps for one tool.
#[derive(Default)]
struct ToolTotals {
    /// Recorded tool calls.
    calls: u64,
    /// Recorded tool results.
    results: u64,
    /// Estimated result tokens.
    tokens: u64,
    /// One estimated-token sample per result.
    samples: Vec<u64>,
    /// Results that reported an error.
    errors: u64,
}

/// The counters the report keeps for one bash command.
#[derive(Default)]
struct BashTotals {
    /// Recorded calls of this command.
    calls: u64,
    /// Estimated result tokens attributed to this command.
    tokens: u64,
    /// One estimated-token sample per attributed result.
    samples: Vec<u64>,
}

/// Everything one command run aggregates.
#[derive(Default)]
struct Report {
    /// Tool counters in first-seen order.
    tools: IndexMap<String, ToolTotals>,
    /// Bash command counters in first-seen order.
    bash: IndexMap<String, BashTotals>,
    /// The bash command of each recorded call by identifier, replaced by a later call.
    calls: HashMap<String, Option<String>>,
    /// Lines the JSON parser rejected.
    parse_errors: u64,
    /// Session files read.
    files: usize,
}

/// Write the tool report for the requested session root and output file.
///
/// # Errors
/// Returns the native directory, read or write failure unchanged, and rejects a
/// recorded null entry the way the report's field reads do.
pub fn main(
    args: &[String],
    now: &dyn Fn() -> chrono::DateTime<chrono::Local>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    main_with_opener(args, now, &platform_opener, stdout, stderr)
}

/// Write the report and hand its path to `opener`.
///
/// The opener is supplied so a caller observes the path without starting a
/// platform viewer.
///
/// # Errors
/// Returns the native directory, read or write failure unchanged, and rejects a
/// recorded null entry the way the report's field reads do.
pub fn main_with_opener(
    args: &[String],
    now: &dyn Fn() -> chrono::DateTime<chrono::Local>,
    opener: &dyn Fn(&str),
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let _ = stderr;
    let Some(options) = parse_options(args, stdout)? else {
        return Ok(ExitCode::SUCCESS);
    };
    if !std::path::Path::new(&options.sessions).exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Sessions directory not found: {}", options.sessions),
        ));
    }
    let files = journals(&options.sessions)?;
    let report = collect(&files)?;
    let page = template::render(&report, &options.sessions, now())?;
    if let Some(directory) = std::path::Path::new(&options.output).parent() {
        std::fs::create_dir_all(directory)?;
    }
    std::fs::write(&options.output, page)?;
    writeln!(stdout, "Wrote {}", options.output)?;
    opener(&options.output);
    Ok(ExitCode::SUCCESS)
}

/// The selected session root and output file.
struct Options {
    /// The resolved session root.
    sessions: String,
    /// The resolved report file.
    output: String,
}

/// Read the arguments, printing help when it is requested.
///
/// # Errors
/// Returns the native path-resolution or write failure unchanged.
fn parse_options(args: &[String], stdout: &mut dyn Write) -> io::Result<Option<Options>> {
    let mut sessions = None;
    let mut output = None;
    let mut index = 0;
    while let Some(arg) = args.get(index) {
        let value = args.get(index + 1).filter(|value| !value.is_empty());
        match arg.as_str() {
            "--sessions-dir" if value.is_some() => {
                sessions = Some(resolve_path(&args[index + 1])?);
                index += 1;
            }
            "--output" | "-o" if value.is_some() => {
                output = Some(resolve_path(&args[index + 1])?);
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
    let sessions = match sessions {
        Some(sessions) => sessions,
        None => session_base()?,
    };
    let output = output.unwrap_or_else(|| {
        let temp = std::env::temp_dir().to_string_lossy().into_owned();
        maestro_path::join(&[&temp, "maestro-tool-stats.html"])
    });
    Ok(Some(Options { sessions, output }))
}

/// Every journal beneath `directory`, in directory and file-name order.
///
/// # Errors
/// Returns the native listing failure unchanged.
fn journals(directory: &str) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    for entry in maestro_listing::read_dir(directory)? {
        let path = entry.path().to_string_lossy().into_owned();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            found.extend(journals(&path)?);
        } else if file_type.is_file() && super::is_journal(&entry.file_name().to_string_lossy()) {
            found.push(path);
        }
    }
    Ok(found)
}

/// Aggregate every selected journal in one pass.
///
/// # Errors
/// Returns the native read failure unchanged, and rejects a null recorded entry.
fn collect(files: &[String]) -> io::Result<Report> {
    let mut report = Report {
        files: files.len(),
        ..Report::default()
    };
    for file in files {
        let bytes = std::fs::read(file)?;
        let content = String::from_utf8_lossy(&bytes);
        for line in kept_lines(&content) {
            let Ok(entry) = serde_json::from_str::<Value>(line) else {
                report.parse_errors += 1;
                continue;
            };
            accumulate(&mut report, &entry)?;
        }
    }
    Ok(report)
}

/// Add one recorded entry's calls and results.
///
/// # Errors
/// Rejects a recorded entry that is neither an object nor a null.
fn accumulate(report: &mut Report, entry: &Value) -> io::Result<()> {
    let Some(entry) = entry.as_object() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Cannot read properties of null (reading 'type')",
        ));
    };
    if entry.get("type").and_then(Value::as_str) != Some("message") {
        return Ok(());
    }
    let Some(message) = entry
        .get("message")
        .filter(|message| maestro_json::is_truthy(message))
    else {
        return Ok(());
    };
    let role = message.get("role").and_then(Value::as_str);
    match role {
        Some("assistant") => calls(report, message),
        Some("toolResult") => results(report, message),
        _ => {}
    }
    Ok(())
}

/// Record the tool calls of one assistant message.
fn calls(report: &mut Report, message: &Value) {
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    for block in blocks {
        let Some(block) = block.as_object() else {
            continue;
        };
        if block.get("type").and_then(Value::as_str) != Some("toolCall") {
            continue;
        }
        let Some(name) = block.get("name").and_then(Value::as_str) else {
            continue;
        };
        report.tools.entry(name.to_owned()).or_default().calls += 1;
        let command = (name == "bash")
            .then(|| block.get("arguments").and_then(bash_command))
            .flatten()
            .filter(|command| !command.is_empty());
        let identifier = block.get("id").and_then(key).map(Cow::into_owned);
        if let Some(command) = command {
            report.bash.entry(command_key(&command)).or_default().calls += 1;
            if let Some(identifier) = identifier {
                report.calls.insert(identifier, Some(command));
                continue;
            }
        }
        if let Some(identifier) = identifier {
            report.calls.insert(identifier, None);
        }
    }
}

/// Record one tool result and attribute its tokens to the call it answers.
fn results(report: &mut Report, message: &Value) {
    let Some(name) = message
        .get("toolName")
        .filter(|name| maestro_json::is_truthy(name))
        .and_then(Value::as_str)
    else {
        return;
    };
    let text = content_text(message.get("content"));
    let tokens = estimate_tokens(&text);
    let totals = report.tools.entry(name.to_owned()).or_default();
    totals.results += 1;
    totals.tokens += tokens;
    totals.samples.push(tokens);
    if message.get("isError") == Some(&Value::Bool(true)) {
        totals.errors += 1;
    }
    let answered = message
        .get("toolCallId")
        .filter(|identifier| maestro_json::is_truthy(identifier))
        .and_then(key)
        .and_then(|identifier| report.calls.get(identifier.as_ref()))
        .and_then(Option::as_deref);
    let Some(command) = answered else {
        return;
    };
    let totals = report.bash.entry(command_key(command)).or_default();
    totals.tokens += tokens;
    totals.samples.push(tokens);
}

/// The map key a recorded identifier contributes, when it is a scalar.
fn key(value: &Value) -> Option<Cow<'_, str>> {
    match value {
        Value::String(text) => Some(Cow::Borrowed(text)),
        Value::Number(number) => Some(Cow::Owned(
            ryu_js::Buffer::new()
                .format_finite(number.as_f64().unwrap_or_default())
                .to_owned(),
        )),
        _ => None,
    }
}

/// The bash command a recorded call named, when the field is a string.
fn bash_command(arguments: &Value) -> Option<String> {
    arguments
        .as_object()
        .and_then(|arguments| arguments.get("command"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The text one result's content contributes, blocks separated by LF.
fn content_text(content: Option<&Value>) -> Cow<'_, str> {
    let Some(content) = content else {
        return Cow::Borrowed("");
    };
    match content {
        Value::String(text) => Cow::Borrowed(text),
        Value::Array(blocks) => {
            Cow::Owned(blocks.iter().map(block_text).collect::<Vec<_>>().join("\n"))
        }
        _ => Cow::Borrowed(""),
    }
}

/// The text one content block contributes.
fn block_text(block: &Value) -> String {
    let text =
        block
            .as_object()
            .and_then(|object| match object.get("type").and_then(Value::as_str) {
                Some("text") => object.get("text").and_then(Value::as_str),
                Some("image") => object.get("data").and_then(Value::as_str),
                _ => None,
            });
    text.map_or_else(|| compact(block), str::to_owned)
}

/// The compact JSON text of one value, as an unknown block contributes it.
fn compact(value: &Value) -> String {
    maestro_json::compact_json(value).unwrap_or_default()
}

/// The report's token estimate of one text, counted as the models crate counts a conversation.
fn estimate_tokens(text: &str) -> u64 {
    tokens_for_chars(utf16_units(text))
        .to_u64()
        .unwrap_or(u64::MAX)
}

/// The best-effort command key of one bash command.
fn command_key(command: &str) -> String {
    let first = first_segment(command);
    let words = ws_words(first.trim_matches(maestro_text::is_whitespace_scalar));
    let mut rest = words.as_slice();
    while rest.len() > 1 && rest.first().is_some_and(|word| assignment(word)) {
        rest = &rest[1..];
    }
    if rest.len() > 1 && rest.first().is_some_and(|word| *word == "sudo") {
        rest = &rest[1..];
    }
    let mut parts = rest.iter();
    let Some(bin) = parts.next() else {
        return "unknown".to_owned();
    };
    match parts.next().filter(|word| !word.starts_with('-')) {
        Some(sub) => format!("{bin} {sub}"),
        None => (*bin).to_owned(),
    }
}

/// The first command segment, ended by a line break, `&&`, `||`, `;` or one pipe.
fn first_segment(command: &str) -> &str {
    let bytes = command.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let rest = &bytes[index..];
        if rest.starts_with(b"\n")
            || rest.starts_with(b"&&")
            || rest.starts_with(b"||")
            || rest.starts_with(b";")
            || rest.starts_with(b"|")
        {
            return &command[..index];
        }
        index += 1;
    }
    command
}

/// The words of `text`, split on the whitespace scalar set.
fn ws_words(text: &str) -> Vec<&str> {
    text.split(maestro_text::is_whitespace_scalar)
        .filter(|word| !word.is_empty())
        .collect()
}

/// Whether `word` is an environment assignment in front of a command.
fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, value)| {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !value.is_empty()
    })
}

/// The histogram of estimated-token samples.
fn histogram(samples: &[u64]) -> Vec<u64> {
    let mut counts = vec![0; BUCKETS.len()];
    for sample in samples {
        // The first upper bound at or above the sample closes the bucket below it.
        let bucket = BUCKETS.iter().position(|bound| *bound >= *sample);
        counts[bucket.map_or(BUCKETS.len() - 1, |index| index.saturating_sub(1))] += 1;
    }
    counts
}

/// The bucket labels the page and the histogram rows use.
fn bucket_labels() -> Vec<String> {
    BUCKETS
        .iter()
        .enumerate()
        .map(|(index, low)| match BUCKETS.get(index + 1) {
            Some(high) => format!("{low}-{high}"),
            None => format!("{low}+"),
        })
        .collect()
}

/// Open the written report with the platform's own viewer.
fn platform_opener(path: &str) {
    let (program, arguments) = match std::env::consts::OS {
        "macos" => ("open", vec![path.to_owned()]),
        "windows" => (
            "cmd",
            vec!["/c".to_owned(), "start".to_owned(), path.to_owned()],
        ),
        _ => ("xdg-open", vec![path.to_owned()]),
    };
    let mut command = maestro_process::command(program);
    command.args(arguments);
    drop(crate::spawn(command, None));
}
