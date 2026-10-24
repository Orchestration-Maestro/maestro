//! Extracting recorded transcripts, splitting them into files and analyzing them.

mod analysis;
mod prompts;

use analysis::{Painting, Tone, paint};
use maestro_session::SessionDir;
use std::borrow::Cow;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

/// The largest transcript text one written file holds, in UTF-16 units.
const MAX_UNITS: usize = 100_000;
/// The separator between two sessions inside one file.
const BETWEEN: &str = "\n\n";

/// Extract, write and optionally analyze the transcripts of one project.
///
/// # Errors
/// Returns the native directory, read or write failure unchanged.
pub async fn main(
    args: &[String],
    color: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<ExitCode> {
    let options = options(args)?;
    let painting = if color {
        Painting::Ansi
    } else {
        Painting::Plain
    };
    std::fs::create_dir_all(&options.output)?;
    let sessions = maestro_session::sessions_dir(&super::session_base()?, &options.cwd);
    if !Path::new(sessions.as_str()).exists() {
        writeln!(stderr, "No sessions found for {}", options.cwd)?;
        writeln!(stderr, "Expected: {}", sessions.as_str())?;
        return Ok(ExitCode::from(1));
    }
    let files = journals(&sessions)?;
    writeln!(
        stdout,
        "Found {} session files in {}",
        files.len(),
        sessions.as_str()
    )?;
    let transcripts = transcripts(&sessions, &files)?;
    if transcripts.is_empty() {
        writeln!(stderr, "No transcripts found")?;
        return Ok(ExitCode::from(1));
    }
    let mut streams = Streams {
        painting,
        stdout,
        stderr,
    };
    let written = write_transcripts(&options.output, &transcripts, &mut streams)?;
    writeln!(
        streams.stdout,
        "\nCreated {} transcript file(s) in {}",
        written.len(),
        options.output
    )?;
    if !options.analyze {
        writeln!(
            streams.stdout,
            "\nRun with --analyze to spawn Maestro subagents for pattern analysis."
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    analyze(&options, &written, &mut streams).await
}

/// The report's streams and how it spells emphasis on them.
struct Streams<'a> {
    /// How the printed lines are spelled.
    painting: Painting,
    /// Report output.
    stdout: &'a mut dyn Write,
    /// Diagnostics.
    stderr: &'a mut dyn Write,
}

/// The selected output directory, project working directory and analysis flag.
struct Options {
    /// Whether the transcripts are analyzed after writing.
    analyze: bool,
    /// The resolved output directory.
    output: String,
    /// The resolved project working directory.
    cwd: String,
}

/// Read the arguments: the analysis flag, the first output value and the first path.
///
/// # Errors
/// Returns the native path-resolution failure unchanged.
fn options(args: &[String]) -> io::Result<Options> {
    let analyze = args.iter().any(|arg| arg == "--analyze");
    let output_flag = args.iter().position(|arg| arg == "--output");
    let value = output_flag.and_then(|index| args.get(index + 1));
    let output = match value.filter(|value| !value.is_empty()) {
        Some(value) => super::resolve_path(value)?,
        None => super::resolve_path("./session-transcripts")?,
    };
    let cwd = args
        .iter()
        .enumerate()
        .find(|(index, arg)| {
            let consumed = output_flag.is_some_and(|flag| *index == flag || *index == flag + 1);
            !consumed && !arg.starts_with("--")
        })
        .map_or_else(
            || super::resolve_path("."),
            |(_, arg)| super::resolve_path(arg),
        )?;
    Ok(Options {
        analyze,
        output,
        cwd,
    })
}

/// The session journals of `directory` in UTF-16 name order.
///
/// # Errors
/// Returns the native listing failure unchanged.
fn journals(directory: &SessionDir) -> io::Result<Vec<String>> {
    let mut names = maestro_listing::read_dir(directory.as_str())?
        .into_iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| super::is_journal(name))
        .collect::<Vec<_>>();
    names.sort_by(|left, right| maestro_text::utf16_order(left, right));
    Ok(names)
}

/// The transcript text of every session journal that holds one.
///
/// # Errors
/// Returns the native read failure unchanged.
fn transcripts(directory: &SessionDir, files: &[String]) -> io::Result<Vec<String>> {
    let mut transcripts = Vec::new();
    for name in files {
        let messages = messages(&maestro_path::join(&[directory.as_str(), name]))?;
        if messages.is_empty() {
            continue;
        }
        transcripts.push(format!(
            "=== SESSION: {name} ===\n{}\n=== END SESSION ===",
            messages.join("\n---\n")
        ));
    }
    Ok(transcripts)
}

/// The stored user and assistant messages of one journal, in file order.
///
/// # Errors
/// Returns the native read failure unchanged.
fn messages(path: &str) -> io::Result<Vec<String>> {
    let content = String::from_utf8_lossy(&std::fs::read(path)?).into_owned();
    let mut messages = Vec::new();
    for entry in maestro_session::parse_session_entries(&content) {
        let Some(message) = entry.get("message") else {
            continue;
        };
        let role = message.get("role").and_then(serde_json::Value::as_str);
        if !matches!(role, Some("user" | "assistant")) {
            continue;
        }
        if entry.get("type").and_then(serde_json::Value::as_str) != Some("message") {
            continue;
        }
        let text = text_of(message.get("content"));
        if text
            .trim_matches(maestro_text::is_whitespace_scalar)
            .is_empty()
        {
            continue;
        }
        let role = role.unwrap_or_default().to_uppercase();
        messages.push(format!("[{role}]\n{text}"));
    }
    Ok(messages)
}

/// The text one recorded message content holds, its text blocks joined by LF.
fn text_of(content: Option<&serde_json::Value>) -> Cow<'_, str> {
    let Some(content) = content else {
        return Cow::Borrowed("");
    };
    match content {
        serde_json::Value::String(text) => Cow::Borrowed(text),
        serde_json::Value::Array(blocks) => Cow::Owned(
            blocks
                .iter()
                .filter(|block| {
                    block.get("type").and_then(serde_json::Value::as_str) == Some("text")
                })
                .filter_map(|block| block.get("text").and_then(serde_json::Value::as_str))
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => Cow::Borrowed(""),
    }
}

/// Write the transcripts into numbered files, keeping each under the limit.
///
/// Returns the file names in write order.
///
/// # Errors
/// Returns the native write failure unchanged.
fn write_transcripts(
    directory: &str,
    transcripts: &[String],
    streams: &mut Streams<'_>,
) -> io::Result<Vec<String>> {
    let mut written = Vec::new();
    let mut pending = String::new();
    let mut pending_units = 0;
    let mut index = 0;
    for transcript in transcripts {
        let units = transcript.encode_utf16().count();
        if pending_units > 0 && pending_units + units + BETWEEN.len() > MAX_UNITS {
            written.push(write_one(directory, index, &pending, false, streams)?);
            index += 1;
            pending.clear();
            pending_units = 0;
        }
        if units > MAX_UNITS {
            written.push(write_one(directory, index, transcript, true, streams)?);
            index += 1;
            continue;
        }
        if pending_units > 0 {
            pending.push_str(BETWEEN);
            pending_units += BETWEEN.len();
        }
        pending.push_str(transcript);
        pending_units += units;
    }
    if pending_units > 0 {
        written.push(write_one(directory, index, &pending, false, streams)?);
    }
    Ok(written)
}

/// Write one numbered transcript file and report it.
///
/// # Errors
/// Returns the native write failure unchanged.
fn write_one(
    directory: &str,
    index: usize,
    content: &str,
    oversized: bool,
    streams: &mut Streams<'_>,
) -> io::Result<String> {
    let name = format!("session-transcripts-{index:03}.txt");
    std::fs::write(maestro_path::join(&[directory, &name]), content)?;
    let report = format!(
        "Wrote {name} ({} chars){}",
        content.encode_utf16().count(),
        if oversized { " - oversized" } else { "" }
    );
    if oversized {
        writeln!(
            streams.stdout,
            "{}",
            paint(streams.painting, Tone::Warning, &report)
        )?;
    } else {
        writeln!(streams.stdout, "{report}")?;
    }
    Ok(name)
}

/// Analyze each written transcript, then aggregate the summaries.
///
/// # Errors
/// Returns the native read, write or child failure unchanged.
async fn analyze(
    options: &Options,
    written: &[String],
    streams: &mut Streams<'_>,
) -> io::Result<ExitCode> {
    let agents = instructions(&options.cwd)?;
    let prompt = prompts::analysis(&agents);
    writeln!(streams.stdout, "\nSpawning subagents for analysis...")?;
    for name in written {
        analyze_one(options, name, &prompt, streams).await?;
    }
    let summaries = summaries(&options.output)?;
    writeln!(streams.stdout, "\n=== Individual Analysis Complete ===")?;
    writeln!(streams.stdout, "Created {} summary files", summaries.len())?;
    if summaries.is_empty() {
        writeln!(
            streams.stdout,
            "{}",
            paint(
                streams.painting,
                Tone::Warning,
                "No summary files created. Nothing to aggregate."
            )
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    aggregate(options, &agents, &summaries, streams).await
}

/// Run the analysis child for one written transcript file.
///
/// # Errors
/// Returns the native read or write failure unchanged.
async fn analyze_one(
    options: &Options,
    name: &str,
    prompt: &str,
    streams: &mut Streams<'_>,
) -> io::Result<()> {
    let path = maestro_path::join(&[&options.output, name]);
    let summary = summary_name(name);
    let summary_path = maestro_path::join(&[&options.output, &summary]);
    let content = String::from_utf8_lossy(&std::fs::read(&path)?).into_owned();
    writeln!(
        streams.stdout,
        "Analyzing {name} ({} chars)...",
        content.encode_utf16().count()
    )?;
    let lines = content.split('\n').count();
    let full = format!(
        "{prompt}\n\nThe file {path} has {lines} lines. Read it in full using chunked reads, then write your analysis to {summary_path}"
    );
    let success = analysis::run(
        &full,
        &options.output,
        streams.painting,
        streams.stdout,
        streams.stderr,
    )
    .await?;
    if success && Path::new(&summary_path).exists() {
        let line = format!("  -> {summary}");
        writeln!(
            streams.stdout,
            "{}",
            paint(streams.painting, Tone::Done, &line)
        )?;
    } else if success {
        let line = format!("Agent finished but did not write {summary}");
        writeln!(
            streams.stderr,
            "{}",
            paint(streams.painting, Tone::Warning, &line)
        )?;
    } else {
        let line = format!("Failed to analyze {name}");
        writeln!(
            streams.stderr,
            "{}",
            paint(streams.painting, Tone::Error, &line)
        )?;
    }
    Ok(())
}

/// Run the aggregation child over every summary file already in the output directory.
///
/// # Errors
/// Returns the native write failure unchanged.
async fn aggregate(
    options: &Options,
    agents: &[String],
    summaries: &[String],
    streams: &mut Streams<'_>,
) -> io::Result<ExitCode> {
    writeln!(
        streams.stdout,
        "\nAggregating findings into final summary..."
    )?;
    let paths = summaries
        .iter()
        .map(|name| maestro_path::join(&[&options.output, name]))
        .collect::<Vec<_>>();
    let final_path = maestro_path::join(&[&options.output, "FINAL-SUMMARY.txt"]);
    let prompt = prompts::aggregation(agents, &paths, &final_path);
    let success = analysis::run(
        &prompt,
        &options.output,
        streams.painting,
        streams.stdout,
        streams.stderr,
    )
    .await?;
    if success && Path::new(&final_path).exists() {
        let created = paint(
            streams.painting,
            Tone::Done,
            "\n=== Final Summary Created ===",
        );
        writeln!(streams.stdout, "{created}")?;
        let line = format!("  {final_path}");
        writeln!(
            streams.stdout,
            "{}",
            paint(streams.painting, Tone::Done, &line)
        )?;
    } else if success {
        let line = "Agent finished but did not write final summary";
        writeln!(
            streams.stderr,
            "{}",
            paint(streams.painting, Tone::Warning, line)
        )?;
    } else {
        let line = "Failed to create final summary";
        writeln!(
            streams.stderr,
            "{}",
            paint(streams.painting, Tone::Error, line)
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

/// The existing instruction files the children may compare against.
///
/// # Errors
/// Returns the native home-directory failure.
fn instructions(cwd: &str) -> io::Result<Vec<String>> {
    let global = maestro_path::join(&[&super::agent_base()?, "AGENTS.md"]);
    let local = maestro_path::join(&[cwd, "AGENTS.md"]);
    Ok([global, local]
        .into_iter()
        .filter(|path| Path::new(path).exists())
        .collect())
}

/// The summary files already present in `directory`, in UTF-16 name order.
///
/// # Errors
/// Returns the native listing failure unchanged.
fn summaries(directory: &str) -> io::Result<Vec<String>> {
    let mut names = maestro_listing::read_dir(directory)?
        .into_iter()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".summary.txt"))
        .collect::<Vec<_>>();
    names.sort_by(|left, right| maestro_text::utf16_order(left, right));
    Ok(names)
}

/// One transcript file's summary file name.
fn summary_name(name: &str) -> String {
    name.replacen(".txt", ".summary.txt", 1)
}
