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
/// An entry that disappears between the listing and its own step is skipped, so a writer
/// that renames its temporary file into a live directory does not fail the caller.
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
        } else {
            link_entry(&entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Links one file into `target`, copying it when the file system cannot link it.
///
/// An entry that a concurrent writer has already renamed is skipped: the listing saw a
/// temporary file that never became part of the directory. Every other failure is returned.
fn link_entry(source: &Path, target: &Path) -> io::Result<()> {
    if std::fs::hard_link(source, target).is_ok() {
        return Ok(());
    }
    match std::fs::copy(source, target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound && source_is_gone(source) => Ok(()),
        result => result.map(drop),
    }
}

/// Report whether `source` itself is gone, rather than only the target's directory.
fn source_is_gone(source: &Path) -> bool {
    std::fs::symlink_metadata(source).is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
}

#[cfg(test)]
mod tests {
    use super::link_entry;
    use maestro_test_directory::Scratch;

    /// A source that a concurrent writer removed before the step is skipped.
    #[test]
    fn an_entry_whose_source_is_gone_is_skipped() {
        let scratch = Scratch::new().unwrap();
        let source = scratch.path().join("component");
        let target = scratch.path().join("linked-component");

        link_entry(&source, &target).unwrap();

        assert!(!target.exists());
    }

    /// A missing destination directory is a failure, not a skip.
    #[test]
    fn a_missing_target_directory_still_fails() {
        let scratch = Scratch::new().unwrap();
        let source = scratch.path().join("component");
        std::fs::write(&source, b"code").unwrap();
        let target = scratch.path().join("absent/linked-component");

        let error = link_entry(&source, &target).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");
        assert_eq!(std::fs::read(&source).unwrap(), b"code");
        assert!(!target.exists());
    }
}
