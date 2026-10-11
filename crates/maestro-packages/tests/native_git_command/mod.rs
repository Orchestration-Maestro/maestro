//! Shared native Git command fixture.
#![cfg(test)]
/// Runs Git without inherited author or signing settings.
pub fn git(path: &std::path::Path, args: &[&str]) -> String {
    let result = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(result.status.success(), "fixture Git command failed");
    String::from_utf8(result.stdout).unwrap().trim().into()
}
