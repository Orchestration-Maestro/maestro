//! Finite reftable repository queries answered without a working Git installation.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use maestro_test_process::child::{leave, write_stdout};
use serde::Deserialize;

/// The status every query outside this finite set ends with.
const REFUSED: i32 = 2;

/// The symbolic branch the alias reports, with the surrounding spaces Git readers trim.
const TOPIC: &[u8] = b" topic\n";

/// The object identifier the alias reports for the existing reference.
const OBJECT: &[u8] = b" 0123456789abcdef\n";

/// Output that must be discarded when the query fails.
const DISCARDED: &[u8] = b"discard this output\n";

/// The checkout every query must name through `-C`, parsed before any output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// The exact `-C` directory of every accepted query.
    cwd: PathBuf,
}

impl Record {
    /// Reject an unknown family or extra fields before writing any output.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("git-probe\n")
            .ok_or_else(|| io::Error::other("missing git-probe family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Answer one of three finite queries and refuse every other operand sequence.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    let Ok(words) = words(operands) else {
        leave(REFUSED)
    };
    let [flag, directory, query @ ..] = words.as_slice() else {
        leave(REFUSED)
    };
    if *flag != "-C" || Path::new(*directory) != record.cwd.as_path() {
        leave(REFUSED);
    }
    match query {
        ["symbolic-ref", "--quiet", "--short", "HEAD"] => answer(TOPIC, 0),
        ["rev-parse", "--verify", "HEAD"] => answer(OBJECT, 0),
        ["rev-parse", "--verify", "missing"] => answer(DISCARDED, 1),
        _ => leave(REFUSED),
    }
}

/// Split operands into text; a non-text word can name no accepted query.
fn words(operands: &[OsString]) -> io::Result<Vec<&str>> {
    operands
        .iter()
        .map(|word| {
            word.to_str()
                .ok_or_else(|| io::Error::other("non-text operand"))
        })
        .collect()
}

/// Write one answer and leave with its status.
fn answer(bytes: &[u8], code: i32) -> io::Result<()> {
    write_stdout(bytes)?;
    leave(code)
}
