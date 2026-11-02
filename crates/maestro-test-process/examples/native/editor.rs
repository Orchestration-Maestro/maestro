//! Controlled editor payload, installed under the editor name the product resolves.

use std::ffi::{OsStr, OsString};
use std::io;

/// The record of the controlled editor, which carries no fields.
pub(super) struct Record;

impl Record {
    /// Read an editor record, rejecting fields it does not take.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let fields: Vec<_> = text.lines().collect();
        let ["editor"] = fields.as_slice() else {
            return Err(io::Error::other("expected the editor record"));
        };
        Ok(Self)
    }
}

/// Skip the editor's own `--custom` option, report the file it was asked to edit, then end
/// with status 7.
pub(super) fn run(_record: &Record, operands: &[OsString]) -> io::Result<()> {
    let mut operands = operands.iter();
    let custom = |operand: &OsString| operand == OsStr::new("--custom");
    let mut edited = operands.next();
    if edited.is_some_and(custom) {
        edited = operands.next();
    }
    let edited = edited.ok_or_else(|| io::Error::other("no file to edit"))?;
    let result = std::env::var_os("MAESTRO_EDITOR_RESULT")
        .ok_or_else(|| io::Error::other("MAESTRO_EDITOR_RESULT is unset"))?;
    std::fs::write(result, [edited.as_encoded_bytes(), b"\n"].concat())?;
    std::process::exit(7)
}
