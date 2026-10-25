//! Hyperlink option and URI byte decoding.

use super::super::{Command, Parser};

/// Read options in order, stopping at an incomplete key/value field.
pub(in crate::osc) fn parse(parser: &mut Parser, _: Option<u8>) -> Option<Command> {
    let data = parser.terminated()?;
    let separator = data.iter().position(|b| *b == b';')?;
    let uri = &data[separator + 1..];
    let mut id = None;
    for field in data[..separator].split(|b| *b == b':') {
        let Some(equal) = field.iter().position(|b| *b == b'=') else {
            break;
        };
        let value = &field[equal + 1..];
        if &field[..equal] == b"id" && !value.is_empty() {
            id = Some(value.to_vec());
        }
    }
    if uri.is_empty() {
        return if id.is_none() {
            Some(Command::HyperlinkEnd)
        } else {
            None
        };
    }
    Some(Command::HyperlinkStart {
        id,
        uri: uri.to_vec(),
    })
}
