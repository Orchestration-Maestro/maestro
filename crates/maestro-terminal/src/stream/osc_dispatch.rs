//! Projection of owned operating-system commands without executing effects.

use super::{Action, Handler};
use crate::osc::Command;

/// Preserve owned command payloads, rejecting only invalid title UTF-8.
pub(super) fn dispatch(command: Command, handler: &mut impl Handler) {
    let action = match command {
        Command::ChangeWindowTitle(title) => {
            if std::str::from_utf8(&title).is_err() {
                return;
            }
            Action::WindowTitle(title)
        }
        Command::SemanticPrompt(command) => Action::SemanticPrompt(command),
        Command::ClipboardContents { kind, data } => Action::ClipboardContents { kind, data },
        Command::ReportPwd { value } => Action::ReportPwd(value),
        Command::HyperlinkStart { id, uri } => Action::StartHyperlink { id, uri },
        Command::HyperlinkEnd => Action::EndHyperlink,
        Command::ColorOperation {
            op,
            requests,
            terminator,
        } => Action::ColorOperation {
            op,
            requests,
            terminator,
        },
        Command::KittyColorProtocol(command) => Action::KittyColorReport(command),
    };
    handler.vt(action);
}
