//! Agent identity retention across process observations.

use maestro_detection::agents::Agent;

/// Consecutive misses required to release an agent identity.
pub const AGENT_MISS_CONFIRMATION_ATTEMPTS: u8 = 6;

/// Retained identity and its missing-process confirmation count.
#[derive(Debug, Clone, Copy)]
pub struct AgentDetectionPresence {
    /// Currently retained identity.
    current_agent: Option<Agent>,
    /// Consecutive process scans that did not identify an agent.
    consecutive_misses: u8,
}

impl AgentDetectionPresence {
    /// Starts tracking a supplied identity without pending misses.
    #[must_use]
    pub fn from_agent(current_agent: Option<Agent>) -> Self {
        Self {
            current_agent,
            consecutive_misses: 0,
        }
    }

    /// Returns the retained identity.
    #[must_use]
    pub fn current_agent(&self) -> Option<Agent> {
        self.current_agent
    }

    /// Clears identity and miss count, returning whether identity changed.
    pub fn clear_current_agent(&mut self) -> bool {
        self.consecutive_misses = 0;
        self.current_agent.take().is_some()
    }

    /// Applies a scan result, retaining identity until the sixth consecutive miss.
    pub fn observe_process_probe(&mut self, identified: Option<Agent>) -> bool {
        if let Some(agent) = identified {
            self.consecutive_misses = 0;
            if self.current_agent == Some(agent) {
                return false;
            }
            self.current_agent = Some(agent);
            return true;
        }
        if self.current_agent.is_none() {
            self.consecutive_misses = 0;
            return false;
        }
        self.consecutive_misses = self.consecutive_misses.saturating_add(1);
        if self.consecutive_misses < AGENT_MISS_CONFIRMATION_ATTEMPTS {
            return false;
        }
        self.clear_current_agent()
    }
}

/// Identity transition resulting from a foreground scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundShellAgentAction {
    /// Apply the ordinary miss-confirmation rule.
    ObserveProbe,
    /// Publish completion before clearing identity.
    ReportProcessExit,
    /// Treat a same-agent return after completion as a new process.
    ReportReplacementProcess,
    /// Release identity after completion has been published.
    ClearAgent,
}

/// Identity observations used to order completion and identity release.
#[derive(Debug, Clone, Copy)]
pub struct ForegroundShellObservation {
    /// Previously retained identity.
    pub previous_agent: Option<Agent>,
    /// Identity recognized by the current scan.
    pub new_agent: Option<Agent>,
    /// Whether the foreground job contains the pane shell.
    pub foreground_is_pane_shell: bool,
    /// Whether completion for the previous process was already published.
    pub process_exit_reported: bool,
}

/// Decides completion ordering without clearing identity prematurely.
#[must_use]
pub fn foreground_shell_agent_action(
    input: ForegroundShellObservation,
) -> ForegroundShellAgentAction {
    use ForegroundShellAgentAction::{
        ClearAgent, ObserveProbe, ReportProcessExit, ReportReplacementProcess,
    };
    let Some(previous) = input.previous_agent else {
        return ObserveProbe;
    };
    if input.process_exit_reported {
        return if input.new_agent == Some(previous) {
            ReportReplacementProcess
        } else if input.new_agent.is_none() {
            ClearAgent
        } else {
            ObserveProbe
        };
    }
    if input.new_agent.is_none() && input.foreground_is_pane_shell {
        return ReportProcessExit;
    }
    ObserveProbe
}

/// Publication state for the shell-return completion transition.
#[derive(Debug, Default)]
pub struct ForegroundShellState {
    /// Whether completion must be published before the next identity scan.
    pub pending_clear: bool,
    /// Whether completion was published for the retained identity.
    pub exit_reported: bool,
}

impl ForegroundShellState {
    /// Applies a scan action, returning whether the retained process identity changed.
    pub fn apply(
        &mut self,
        presence: &mut AgentDetectionPresence,
        input: ForegroundShellObservation,
    ) -> bool {
        match foreground_shell_agent_action(input) {
            ForegroundShellAgentAction::ReportProcessExit => {
                self.pending_clear = true;
                false
            }
            action => {
                self.pending_clear = false;
                self.exit_reported = false;
                match action {
                    ForegroundShellAgentAction::ReportReplacementProcess => {
                        presence.observe_process_probe(input.previous_agent);
                        true
                    }
                    ForegroundShellAgentAction::ClearAgent => presence.clear_current_agent(),
                    _ => presence.observe_process_probe(input.new_agent),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
