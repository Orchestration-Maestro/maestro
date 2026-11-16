//! The native directory behind a component path is always a directory.
use super::*;

/// A file reached where a directory belongs is refused, as the trimmed `<file>\.` reaches it on Windows.
#[test]
fn a_file_never_stands_for_the_directory_of_a_path() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let file = scratch.path().join("victim");
    std::fs::write(&file, b"keep").unwrap();
    let refused = directory(&file).err().map(|error| error.kind());
    assert_eq!(refused, Some(io::ErrorKind::NotADirectory));
    assert!(directory(scratch.path()).is_ok());
    assert_eq!(std::fs::read(&file).unwrap(), b"keep");
}
