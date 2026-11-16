//! Shared native Git command fixture.
#![cfg(test)]
/// Runs Git without inherited author or signing settings.
pub fn git(path: &std::path::Path, args: &[&str]) -> String {
    let result = maestro_test_harness::git_command::command()
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "fixture Git command {args:?} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
