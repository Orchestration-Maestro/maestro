//! The agent directory a run shares with its children.

use std::io;
use std::path::Path;

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

/// Links every file below `from` into the same place below `to`, copying a file the file system
/// cannot link. Does nothing when `from` is not a directory, so a run that prepared nothing
/// leaves `to` unmade.
///
/// A test that gives its own agent directory to a component host links the run's compiled code
/// into it this way, so it loads the code the run prepared instead of compiling it again.
///
/// # Errors
/// Returns the first failure to read `from`, create a directory below `to` or place a file.
pub fn link_files(from: &Path, to: &Path) -> io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            link_files(&entry.path(), &target)?;
        } else if std::fs::hard_link(entry.path(), &target).is_err() {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
