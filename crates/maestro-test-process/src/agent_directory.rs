//! The agent directory a run shares with its children.

/// The variable that names the agent directory. Compiled components cache their code below it,
/// so a child that names the run's directory reuses the code the run's setup prepared.
pub const AGENT_DIRECTORY: &str = "MAESTRO_CODING_AGENT_DIR";

/// The agent directory the run's setup exported, when it is an absolute path.
///
/// A child that replaces `HOME` or clears its environment passes this value back under
/// [`AGENT_DIRECTORY`], so it does not compile again the components the run prepared.
#[must_use]
pub fn run_agent_directory() -> Option<String> {
    std::env::var(AGENT_DIRECTORY)
        .ok()
        .filter(|path| std::path::Path::new(path).is_absolute())
}
