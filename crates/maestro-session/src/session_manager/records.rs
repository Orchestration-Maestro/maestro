//! Record framing shared by whole-text parsing and streamed session reads.
use super::{FileEntry, context::ecma_whitespace};
use maestro_json::jsonl::JsonlLineSplitter;
use std::{
    io::{self, BufRead, ErrorKind},
    ops::ControlFlow,
};

/// Which parsed JSON values a reader admits as records.
#[derive(Clone, Copy)]
pub(super) enum Admission {
    /// Keep every JSON value for whole-text parsing.
    All,
    /// Skip null, false, zero and empty strings in stored histories.
    Nonempty,
}

/// Hand each record of `reader` to `on_entry` until it breaks or the input ends.
///
/// Lines come from the shared JSONL splitter, so only the current line and the last
/// nonblank one are held. Blank and malformed lines are skipped; `admission` selects
/// whether empty JSON values are kept. The outer ECMAScript
/// whitespace of the first and last nonblank lines is dropped, as whole-text parsing
/// drops it.
pub(super) fn for_each_entry(
    mut reader: impl BufRead,
    admission: Admission,
    mut on_entry: impl FnMut(FileEntry) -> ControlFlow<()>,
) -> io::Result<()> {
    let mut splitter = JsonlLineSplitter::default();
    let mut held: Option<String> = None;
    let mut first = true;
    let mut accept = |line: String| {
        if line.trim_matches(ecma_whitespace).is_empty() {
            return ControlFlow::Continue(());
        }
        let line = if first {
            line.trim_start_matches(ecma_whitespace).to_owned()
        } else {
            line
        };
        first = false;
        held.replace(line)
            .map_or(ControlFlow::Continue(()), |previous| {
                parse_line(&previous, admission, &mut on_entry)
            })
    };
    loop {
        let chunk = match reader.fill_buf() {
            Ok(chunk) => chunk,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if chunk.is_empty() {
            break;
        }
        let count = chunk.len();
        for line in splitter.push(chunk) {
            if accept(line).is_break() {
                return Ok(());
            }
        }
        reader.consume(count);
    }
    if let Some(tail) = splitter.finish()
        && accept(tail).is_break()
    {
        return Ok(());
    }
    if let Some(last) = held {
        // Nothing follows the last record, so its break has nothing left to stop.
        let _ = parse_line(
            last.trim_end_matches(ecma_whitespace),
            admission,
            &mut on_entry,
        );
    }
    Ok(())
}

/// Hand an admitted record on `line` to `on_entry`; malformed lines are skipped.
fn parse_line(
    line: &str,
    admission: Admission,
    on_entry: &mut impl FnMut(FileEntry) -> ControlFlow<()>,
) -> ControlFlow<()> {
    let Ok(entry) = serde_json::from_str(line) else {
        return ControlFlow::Continue(());
    };
    if matches!(admission, Admission::Nonempty) && !maestro_json::is_truthy(&entry) {
        return ControlFlow::Continue(());
    }
    on_entry(entry)
}
