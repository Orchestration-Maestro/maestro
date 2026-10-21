//! Shared presentation ordering for public daemon observations.

use maestro_detection::agents::AgentState;

/// Orders blocked, unacknowledged completion, active, idle and unknown panes.
pub(super) fn tab_attention_priority(state: AgentState, seen: bool) -> u8 {
    match (state, seen) {
        (AgentState::Blocked, _) => 4,
        (AgentState::Idle, false) => 3,
        (AgentState::Working, _) => 2,
        (AgentState::Idle, true) => 1,
        (AgentState::Unknown, _) => 0,
    }
}
