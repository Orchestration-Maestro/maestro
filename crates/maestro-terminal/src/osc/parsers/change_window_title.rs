//! Window title byte capture.

use super::super::{Command, Parser};

/// Retain a title after reserving its sentinel.
pub(in crate::osc) fn parse(parser: &mut Parser, _: Option<u8>) -> Option<Command> {
    Some(Command::ChangeWindowTitle(parser.terminated()?.to_vec()))
}
