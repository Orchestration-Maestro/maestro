//! Direct shell execution with cleaned rolling output and sanitized overflow artifacts.
use super::bash::{BashError, BashExecOptions, BashOperations};
use crate::output::{
    output_accumulator::{close_artifact, open_artifact},
    render_utils::{AnsiStream, normalize_display_text},
    truncate::{DEFAULT_MAX_BYTES, TruncationOptions, truncate_tail},
};
use crate::utils::shell::sanitize_binary_output;
use maestro_models::{Cancellation, Local, Threaded};
use maestro_text::Utf8Stream;
use std::{collections::VecDeque, io::Write};

/// Selected direct output and its execution status.
pub type BashResult = maestro_tool::bash_executor::BashResult<Option<f64>, Option<String>>;
/// Both execution families use the same output and settlement algorithm.
macro_rules! executor {
    ($mode:ty, [$($bound:tt)*], [$($shared:tt)*]) => {
/// Optional cleaned-output callback and cancellation signal.
#[derive(Default)]
pub struct BashExecutorOptions<'a> {
    /// Receives each cleaned raw chunk synchronously, including empty chunks.
    pub on_chunk: Option<&'a mut (dyn FnMut(&str) + 'a $($bound)*)>,
    /// The signal forwarded unchanged to the operation.
    pub signal: Option<&'a Cancellation>,
}
/// Invoke the supplied operation with literal operands and direct output policy.
///
/// # Errors
/// Returns the original ordinary operation error or first output-processing failure.
pub async fn execute_bash_with_operations(
    command: &str,
    cwd: &str,
    operations: &(dyn BashOperations<$mode> $($bound)* $($shared)* + '_),
    mut options: BashExecutorOptions<'_>,
) -> Result<BashResult, BashError> {
    let mut state = Output::default();
    let mut failure = None;
    let mut sink = |bytes: &[u8]| {
        if failure.is_none() {
            state.raw_bytes += bytes.len();
            let text = state.decoder.push(bytes);
            failure = state.accept(&text, false, &mut options.on_chunk).err();
        }
    };
    let outcome = operations
        .exec(
            command,
            cwd,
            BashExecOptions {
                on_data: &mut sink,
                signal: options.signal,
                timeout: None,
                env: None,
            },
        )
        .await;
    let cancelled = options.signal.is_some_and(Cancellation::is_aborted);
    let selected = match (failure, outcome) {
        (Some(error), _) => Err(error),
        (None, Err(error)) if !cancelled => Err(error),
        (None, outcome) => state.settle(
            outcome.ok().and_then(|result| result.exit_code),
            options.signal,
            &mut options.on_chunk,
        ),
    };
    let closed = close_artifact(state.writer.take());
    let result = selected?;
    closed?;
    Ok(result)
}
    };
}
executor!(Threaded, [+ Send], [+ Sync]);

/// Caller-local direct execution with the same sanitization and artifact policy.
pub mod local {
    use super::{
        BashError, BashExecOptions, BashOperations, BashResult, Cancellation, Local, Output,
        close_artifact,
    };
    executor!(Local, [], []);
}

/// Direct policy state; artifact contents differ from raw tool-output artifacts.
#[derive(Default)]
struct Output {
    /// Raw UTF-8 carry.
    decoder: Utf8Stream,
    /// Held escape suffix.
    ansi: AnsiStream,
    /// Whether initial BOM selection has run.
    started: bool,
    /// Total received raw bytes.
    raw_bytes: usize,
    /// Whole cleaned chunks in arrival order.
    chunks: VecDeque<String>,
    /// Retained UTF-16 code-unit length.
    units: usize,
    /// Artifact identity after creation.
    path: Option<String>,
    /// Synchronous writer, released at settlement.
    writer: Option<Box<dyn Write + Send>>,
}
impl Output {
    /// Decode caller-owned initial BOM policy and strip escapes before storing text.
    fn accept<S: FnMut(&str) + ?Sized>(
        &mut self,
        text: &str,
        finalizing: bool,
        callback: &mut Option<&mut S>,
    ) -> Result<(), BashError> {
        let text = if !self.started && !text.is_empty() {
            self.started = true;
            text.strip_prefix('\u{feff}').unwrap_or(text)
        } else {
            text
        };
        let mut clean = self.ansi.push(text)?;
        if finalizing {
            clean.push_str(&self.ansi.finish()?);
        }
        let clean = normalize_display_text(&sanitize_binary_output(&clean));
        if !finalizing || !clean.is_empty() {
            self.store(clean, callback)?;
        }
        Ok(())
    }
    /// Spill on raw overflow, then evict whole oldest chunks by UTF-16 units.
    fn store<S: FnMut(&str) + ?Sized>(
        &mut self,
        text: String,
        callback: &mut Option<&mut S>,
    ) -> Result<(), BashError> {
        if self.raw_bytes > DEFAULT_MAX_BYTES {
            self.spill()?;
        }
        if let Some(writer) = &mut self.writer {
            writer.write_all(text.as_bytes())?;
        }
        self.units += text.encode_utf16().count();
        self.chunks.push_back(text);
        while self.units > DEFAULT_MAX_BYTES * 2 && self.chunks.len() > 1 {
            if let Some(oldest) = self.chunks.pop_front() {
                self.units -= oldest.encode_utf16().count();
            }
        }
        if let Some(callback) = callback
            && let Some(last) = self.chunks.back()
        {
            callback(last);
        }
        Ok(())
    }
    /// Open once, replaying the retained sanitized prefix before any crossing chunk.
    fn spill(&mut self) -> Result<(), BashError> {
        if self.writer.is_none() {
            let (path, mut file) = open_artifact("maestro-bash")?;
            for chunk in &self.chunks {
                file.write_all(chunk.as_bytes())?;
            }
            self.path = Some(path);
            self.writer = Some(Box::new(file));
        }
        Ok(())
    }
    /// Flush decoder and escape carry before shared tail selection.
    fn settle<S: FnMut(&str) + ?Sized>(
        &mut self,
        exit_code: Option<f64>,
        signal: Option<&Cancellation>,
        callback: &mut Option<&mut S>,
    ) -> Result<BashResult, BashError> {
        let rest = self.decoder.finish();
        self.accept(&rest, true, callback)?;
        let text: String = self.chunks.iter().map(String::as_str).collect();
        let selected = truncate_tail(&text, TruncationOptions::default());
        if selected.truncated {
            self.spill()?;
        }
        let cancelled = signal.is_some_and(Cancellation::is_aborted);
        Ok(BashResult {
            output: selected.content,
            exit_code: if cancelled { None } else { exit_code },
            cancelled,
            truncated: selected.truncated,
            full_output_path: self.path.take(),
        })
    }
}

#[cfg(test)]
mod tests;
