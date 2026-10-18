//! Extended clipboard and working-directory commands.

use super::super::{Command, Parser};

/// Decode the supported keys with ASCII case-insensitive names.
pub(in crate::osc) fn parse(parser: &mut Parser, _: Option<u8>) -> Option<Command> {
    let data = parser.terminated()?;
    let equal = data.iter().position(|b| *b == b'=')?;
    let key = &data[..equal];
    let value = &data[equal + 1..];
    if key.eq_ignore_ascii_case(b"CurrentDir") && !value.is_empty() {
        return Some(Command::ReportPwd {
            value: value.to_vec(),
        });
    }
    if key.eq_ignore_ascii_case(b"Copy") {
        let value = value.strip_prefix(b":")?;
        if !value.is_empty() && value != b"?" {
            return Some(Command::ClipboardContents {
                kind: b'c',
                data: value.to_vec(),
            });
        }
    }
    None
}
