//! Agent catalogue, foreground-process recognition and compiled screen detection.
//!
//! Callers own manifest caches, source roots and catalog transport.

pub mod agents;

pub mod updates;

pub mod definitions;

#[cfg(test)]
mod tests;

/// Detects state for a recognized foreground agent using retained screen rules.
#[must_use]
pub fn detect_agent(
    cache: &definitions::ManifestCache,
    agent: Option<agents::Agent>,
    screen: &str,
) -> agents::AgentDetection {
    detect_agent_with_osc(cache, agent, screen, "", "")
}

/// Detects state with independent screen, title and progress evidence.
#[must_use]
pub fn detect_agent_with_osc(
    cache: &definitions::ManifestCache,
    agent: Option<agents::Agent>,
    screen: &str,
    osc_title: &str,
    osc_progress: &str,
) -> agents::AgentDetection {
    match agent {
        Some(agent) => cache.detect_with_osc(
            agent,
            definitions::DetectionInput {
                screen,
                osc_title,
                osc_progress,
            },
        ),
        None => agents::AgentDetection {
            state: agents::AgentState::Unknown,
            skip_state_update: false,
            visible: agents::AgentVisibility::default(),
        },
    }
}

/// Returns whether a recognized agent's screen suppresses retained-state updates.
#[must_use]
pub fn should_skip_state_update(
    cache: &definitions::ManifestCache,
    agent: Option<agents::Agent>,
    screen: &str,
) -> bool {
    agent.is_some_and(|agent| cache.should_skip_state_update(agent, screen))
}
