//! Clipboard selection and raw payload decoding.

use super::super::{Command, Parser};

/// Read the optional single-byte selection without decoding the contents.
pub(in crate::osc) fn parse(parser: &mut Parser, _: Option<u8>) -> Option<Command> {
    let data = parser.terminated()?;
    let (kind, payload) = if data.first() == Some(&b';') {
        (b'c', &data[1..])
    } else if data.get(1) == Some(&b';') {
        (data[0], &data[2..])
    } else {
        return None;
    };
    Some(Command::ClipboardContents {
        kind,
        data: payload.to_vec(),
    })
}
