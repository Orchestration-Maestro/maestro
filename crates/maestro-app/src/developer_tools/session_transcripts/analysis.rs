//! One analysis child, its event stream and the terminal text it prints.

use maestro_json::jsonl::JsonlLineSplitter;
use maestro_process::child_process::{
    CapturedChild, OutputStream, Termination, WaitOptions, wait_for_child_process_with_sink,
};
use serde_json::Value;
use std::borrow::Cow;
use std::io::{self, Write};

/// The widest preview line the analysis output shows.
const PREVIEW: usize = 100;

/// How the analysis output spells emphasis for its own stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Painting {
    /// Plain text, as a redirected stream receives it.
    Plain,
    /// Terminal colors, as a terminal receives them.
    Ansi,
}

/// The emphasis one printed line carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
    /// Secondary text.
    Dim,
    /// A tool call.
    Accent,
    /// A rejected step.
    Error,
    /// A completed step.
    Done,
    /// A step that finished without its result.
    Warning,
}

/// The program that runs one analysis child.
const PROGRAM: &str = "maestro";

/// Spell `text` in `tone`, as the selected painting renders it.
pub(super) fn paint(painting: Painting, tone: Tone, text: &str) -> String {
    if painting == Painting::Plain {
        return text.to_owned();
    }
    let code = match tone {
        Tone::Dim => "2",
        Tone::Accent => "36",
        Tone::Error => "31",
        Tone::Done => "32",
        Tone::Warning => "33",
    };
    let close = if tone == Tone::Dim { "22" } else { "39" };
    format!("\u{1b}[{code}m{text}\u{1b}[{close}m")
}

/// Run one analysis child in `directory` and print its event stream.
///
/// Returns whether the child exited successfully.
///
/// # Errors
/// Returns the native spawn, wait or output-write failure unchanged.
pub(super) async fn run(
    prompt: &str,
    directory: &str,
    painting: Painting,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> io::Result<bool> {
    let mut command = maestro_process::command(PROGRAM);
    command
        .args(["--mode", "json", "--tools", "read,write", "-p", prompt])
        .current_dir(directory);
    let mut child = match CapturedChild::spawn(&mut command) {
        Ok(child) => child,
        Err(error) => {
            let message = format!("  Failed to spawn {PROGRAM}: {error}");
            writeln!(stderr, "{}", paint(painting, Tone::Error, &message))?;
            return Ok(false);
        }
    };
    let mut stream = Stream {
        splitter: JsonlLineSplitter::default(),
        buffer: String::new(),
        has_text: false,
        painting,
        stdout,
        stderr,
        failure: None,
    };
    let output = {
        let mut sink = |which: OutputStream, bytes: &[u8]| stream.push(which, bytes);
        wait_for_child_process_with_sink(
            &mut child,
            WaitOptions::default(),
            std::future::pending::<Termination>(),
            &mut sink,
        )
        .await
    };
    if let Some(failure) = stream.failure {
        return Err(failure);
    }
    Ok(output?.status.success())
}

/// The accumulated text and the caller's streams of one analysis child.
struct Stream<'a> {
    /// Unfinished bytes of the child's standard output.
    splitter: JsonlLineSplitter,
    /// Text the child produced since its last flushed line.
    buffer: String,
    /// Whether `buffer` holds a non-whitespace scalar.
    has_text: bool,
    /// How the printed lines are spelled.
    painting: Painting,
    /// Report output.
    stdout: &'a mut dyn Write,
    /// Child standard error and diagnostics.
    stderr: &'a mut dyn Write,
    /// The first write failure, reported after the wait completes.
    failure: Option<io::Error>,
}

impl Stream<'_> {
    /// Handle one raw chunk from the child.
    fn push(&mut self, which: OutputStream, bytes: &[u8]) {
        let result = match which {
            OutputStream::Stdout => self.stdout_bytes(bytes),
            OutputStream::Stderr => {
                let text = String::from_utf8_lossy(bytes);
                write!(self.stderr, "{}", paint(self.painting, Tone::Error, &text))
            }
        };
        if let Err(error) = result {
            self.failure.get_or_insert(error);
        }
    }

    /// Split one standard-output chunk into lines and handle each.
    fn stdout_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        for line in self.splitter.push(bytes) {
            self.handle(&line)?;
        }
        Ok(())
    }

    /// Handle one child event line, ignoring malformed or unrelated lines.
    fn handle(&mut self, line: &str) -> io::Result<()> {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        match event.get("type").and_then(Value::as_str) {
            Some("message_update") => {
                self.delta(&event);
                Ok(())
            }
            Some("tool_execution_start") => self.tool(&event),
            Some("turn_end") => self.end_turn(),
            _ => Ok(()),
        }
    }

    /// Accumulate one text delta.
    fn delta(&mut self, event: &Value) {
        let Some(update) = event.get("assistantMessageEvent") else {
            return;
        };
        if update.get("type").and_then(Value::as_str) != Some("text_delta") {
            return;
        }
        let Some(delta) = update.get("delta").and_then(Value::as_str) else {
            return;
        };
        self.buffer.push_str(delta);
        self.has_text |= delta
            .chars()
            .any(|c| !maestro_text::is_whitespace_scalar(c));
    }

    /// Print the pending text, then the starting tool call.
    fn tool(&mut self, event: &Value) -> io::Result<()> {
        let Some(name) = event
            .get("toolName")
            .filter(|name| maestro_json::is_truthy(name))
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        self.flush()?;
        let line = format!("  [{name}] {}", arguments(event, name));
        writeln!(self.stdout, "{}", paint(self.painting, Tone::Accent, &line))
    }

    /// Print the pending text and forget it.
    fn end_turn(&mut self) -> io::Result<()> {
        self.flush()?;
        self.buffer.clear();
        self.has_text = false;
        Ok(())
    }

    /// Print the pending text as one preview line, when it holds any text.
    fn flush(&mut self) -> io::Result<()> {
        if !self.has_text {
            return Ok(());
        }
        let line = format!("  {}", preview(&self.buffer));
        self.buffer.clear();
        self.has_text = false;
        writeln!(self.stdout, "{}", paint(self.painting, Tone::Dim, &line))
    }
}

/// The arguments one starting tool call prints after its name.
fn arguments(event: &Value, name: &str) -> String {
    let Some(args) = event.get("args") else {
        return String::new();
    };
    let field = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or_default();
    match name {
        "read" => {
            let mut text = field("path").to_owned();
            for key in ["offset", "limit"] {
                if let Some(value) = args.get(key).filter(|value| maestro_json::is_truthy(value)) {
                    text.push(' ');
                    text.push_str(key);
                    text.push('=');
                    text.push_str(&spell(value));
                }
            }
            text
        }
        "write" => field("path").to_owned(),
        _ => String::new(),
    }
}

/// Spell one recorded number or string the way a text argument reads.
fn spell(value: &Value) -> Cow<'_, str> {
    match value {
        Value::Number(number) => Cow::Owned(
            ryu_js::Buffer::new()
                .format_finite(number.as_f64().unwrap_or_default())
                .to_owned(),
        ),
        Value::String(text) => Cow::Borrowed(text),
        other => Cow::Owned(maestro_json::compact_json(other).unwrap_or_default()),
    }
}

/// The one-line preview of accumulated text, clipped to whole scalars.
fn preview(text: &str) -> String {
    let mut single = String::new();
    let mut spaced = false;
    for character in text.chars() {
        if maestro_text::is_whitespace_scalar(character) {
            spaced = !single.is_empty();
            continue;
        }
        if spaced {
            single.push(' ');
            spaced = false;
        }
        single.push(character);
    }
    if single.encode_utf16().count() <= PREVIEW {
        return single;
    }
    let mut clipped = maestro_text::truncate_utf16(&single, PREVIEW - 3);
    clipped.push_str("...");
    clipped
}
