//! Notification alias that records its operands and returns a chosen status.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use maestro_test_process::child::leave;
use serde::Deserialize;

/// Operand capture and delivery status of one notification alias.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    /// Newline-separated operand destination; none when delivery only fails.
    args: Option<PathBuf>,
    /// The native status the alias ends with.
    code: u8,
}

impl Record {
    /// Reject an unknown family, extra fields and out-of-range statuses before any write.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("notify-send\n")
            .ok_or_else(|| io::Error::other("missing notify-send family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Record each untouched operand on its own line, then leave with the chosen status.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    if let Some(path) = &record.args {
        let mut file = std::fs::File::create(path)?;
        for operand in operands {
            file.write_all(operand.as_encoded_bytes())?;
            file.write_all(b"\n")?;
        }
    }
    leave(i32::from(record.code))
}
