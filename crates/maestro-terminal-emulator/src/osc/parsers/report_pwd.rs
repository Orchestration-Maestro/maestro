//! Unvalidated directory-report bytes.

use super::super::{Command, Parser};

/// Preserve the complete directory spelling without URL parsing.
pub(in crate::osc) fn parse(parser: &mut Parser, _: Option<u8>) -> Option<Command> {
    Some(Command::ReportPwd {
        value: parser.terminated()?.to_vec(),
    })
}
