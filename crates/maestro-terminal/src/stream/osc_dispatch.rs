//! Projection of owned operating-system commands without executing effects.

use crate::stream::OscAction;

use super::{Action, Handler};
use crate::osc::Command;

/// Preserve owned command payloads, rejecting only invalid title UTF-8.
pub(super) fn dispatch(command: Command, handler: &mut impl Handler) {
    let action = match command {
        Command::ChangeWindowTitle(title) => {
            if std::str::from_utf8(&title).is_err() {
                return;
            }
            Action::Osc(OscAction::WindowTitle(title))
        }
        Command::SemanticPrompt(command) => Action::Osc(OscAction::SemanticPrompt(command)),
        Command::ClipboardContents { kind, data } => {
            Action::Osc(OscAction::ClipboardContents { kind, data })
        }
        Command::ReportPwd { value } => Action::Osc(OscAction::ReportPwd(value)),
        Command::HyperlinkStart { id, uri } => Action::Osc(OscAction::StartHyperlink { id, uri }),
        Command::HyperlinkEnd => Action::Osc(OscAction::EndHyperlink),
        Command::ColorOperation {
            op,
            requests,
            terminator,
        } => Action::Osc(OscAction::ColorOperation {
            op,
            requests,
            terminator,
        }),
        Command::KittyColorProtocol(command) => Action::Osc(OscAction::KittyColorReport(command)),
    };
    handler.vt(action);
}
