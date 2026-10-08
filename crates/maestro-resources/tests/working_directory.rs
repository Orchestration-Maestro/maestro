#![cfg(unix)]

#[cfg(test)]
pub mod support;
use maestro_resources::{NativeResourceOperations, canonicalize_path};
use support::text;

/// Only a relative path needs the working directory, which this test removes.
#[test]
fn maestro_paths_resolve_absolute_paths_without_a_working_directory() {
    let dir = support::Directory::new();
    let file = dir.file("target/skill.md", "hello");
    let alias = dir.0.join("alias");
    std::os::unix::fs::symlink(dir.0.join("target"), &alias).unwrap();
    let gone = dir.0.join("gone");
    std::fs::create_dir(&gone).unwrap();
    std::env::set_current_dir(&gone).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    assert_eq!(
        canonicalize_path(text(&alias.join("skill.md")), &NativeResourceOperations),
        text(&file)
    );
    for relative in ["alias/skill.md", "é.md", "é/ü.md"] {
        assert_eq!(
            canonicalize_path(relative, &NativeResourceOperations),
            relative
        );
    }
}
