//! Subscription wire names and first-declared inbound spelling.

use crate::Tool;
use std::borrow::Cow;

/// Names recognized by the subscription protocol.
const NAMES: [&str; 17] = [
    "Read",
    "Write",
    "Edit",
    "Bash",
    "Grep",
    "Glob",
    "AskUserQuestion",
    "EnterPlanMode",
    "ExitPlanMode",
    "KillShell",
    "NotebookEdit",
    "Skill",
    "Task",
    "TaskOutput",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
];

/// Naming policy selected with the request's authorization.
#[derive(Clone, Copy)]
pub(super) enum Naming {
    /// Keep caller spellings.
    Plain,
    /// Canonicalize recognized subscription names.
    Subscription,
}

impl Naming {
    /// Keep plain names; in subscription mode canonicalize lowercase matches.
    pub(super) fn outbound(self, name: &str) -> Cow<'_, str> {
        if matches!(self, Self::Subscription) {
            let lowered = name.to_lowercase();
            if let Some(canonical) = NAMES
                .iter()
                .find(|candidate| candidate.to_lowercase() == lowered)
            {
                return Cow::Borrowed(canonical);
            }
        }
        Cow::Borrowed(name)
    }
}

/// Restore the first declared lowercase match, leaving unknown incoming names unchanged.
pub(super) fn inbound(name: String, tools: &[Tool]) -> String {
    let lowered = name.to_lowercase();
    tools
        .iter()
        .find(|tool| tool.name.to_lowercase() == lowered)
        .map_or(name, |tool| tool.name.clone())
}
