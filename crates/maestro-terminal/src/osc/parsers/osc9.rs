//! Directory and prompt markers within OSC 9.

use super::super::{Command, Parser, semantic_prompt};

/// Decode only the directory and prompt-start command families.
pub(in crate::osc) fn parse(parser: &mut Parser, _: Option<u8>) -> Option<Command> {
    if parser.data().starts_with(b"12") {
        return Some(Command::SemanticPrompt(semantic_prompt::Command::init(
            semantic_prompt::Action::FreshLineNewPrompt,
        )));
    }
    if !parser.data().starts_with(b"9;") {
        return None;
    }
    Some(Command::ReportPwd {
        value: parser.terminated()?[2..].to_vec(),
    })
}
