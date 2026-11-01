//! Native Git fixtures shared by application and compiled-operation tests.
#[path = "../../../maestro-packages/tests/native_git_command/mod.rs"]
mod commands;
pub use commands::git;

/// Creates two commits with a pin at the first and a local-only origin.
pub fn pinned_repository(path: &std::path::Path) -> String {
    std::fs::create_dir_all(path).unwrap();
    git(path, &["init", "-b", "main"]);
    git(path, &["remote", "add", "origin", "."]);
    std::fs::write(path.join("content"), "pinned").unwrap();
    std::fs::write(path.join("retained"), "retained").unwrap();
    git(path, &["add", "content", "retained"]);
    git(path, &["commit", "-m", "pinned"]);
    let pin = git(path, &["rev-parse", "HEAD"]);
    git(path, &["tag", "pin"]);
    std::fs::write(path.join("content"), "later").unwrap();
    git(path, &["add", "content"]);
    git(path, &["commit", "-m", "later"]);
    pin
}
