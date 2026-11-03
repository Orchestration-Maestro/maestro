//! Finite clipboard child programs selected by a sibling JSON record.

use std::ffi::OsString;
use std::io::{self, Write};

use maestro_test_process::child::{connect, leave, read_input, write_stderr, write_stdout};
use serde::Deserialize;

/// Clipboard scenarios parsed completely before any stream or socket changes.
#[derive(Deserialize)]
#[serde(tag = "scenario", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Record {
    /// Copy standard input to standard output, then the one operand to standard error.
    Echo {
        /// Status after both streams received their bytes.
        code: u8,
    },
    /// Send standard input to a parent-owned loopback port, as a detached writer does.
    Send {
        /// Parent's receiving port.
        port: u16,
    },
    /// Offer `types` for `--list-types` and print `image` for any other request, as a Wayland
    /// clipboard reader does.
    Paste {
        /// Text answering `--list-types`.
        types: String,
        /// Bytes answering every other request.
        image: Vec<u8>,
    },
    /// Write `count` copies of one byte, for a bounded capture's exact limit.
    Fill {
        /// The repeated byte value.
        byte: u8,
        /// How many bytes to write.
        count: usize,
    },
}

impl Record {
    /// Reject unknown scenarios, extra fields and malformed values before effects.
    pub(super) fn parse(text: &str) -> io::Result<Self> {
        let json = text
            .strip_prefix("clipboard\n")
            .ok_or_else(|| io::Error::other("missing clipboard family"))?;
        serde_json::from_str(json).map_err(io::Error::other)
    }
}

/// Run the selected clipboard program without harness bytes or a command parser.
pub(super) fn run(record: &Record, operands: &[OsString]) -> io::Result<()> {
    match record {
        Record::Echo { code } => {
            let [diagnostic] = operands else {
                return Err(io::Error::other("echo expects one operand"));
            };
            write_stdout(&read_input()?)?;
            write_stderr(diagnostic.as_encoded_bytes())?;
            leave(i32::from(*code))
        }
        Record::Send { port } => {
            let mut stream = connect(*port)?;
            stream.write_all(&read_input()?)?;
            leave(0)
        }
        Record::Paste { types, image } => {
            if operands
                .first()
                .is_some_and(|first| first == "--list-types")
            {
                write_stdout(types.as_bytes())
            } else {
                write_stdout(image)
            }
        }
        Record::Fill { byte, count } => write_stdout(&vec![*byte; *count]),
    }
}
