//! Files linked from the run's directory appear in a directory the test owns.

use maestro_test_directory::Scratch;
use maestro_test_process::link_files;

#[test]
fn link_files_reproduces_every_nested_file_and_ignores_a_missing_source() {
    let scratch = Scratch::new().unwrap();
    let from = scratch.path().join("from");
    std::fs::create_dir_all(from.join("modules/namespace")).unwrap();
    std::fs::write(from.join("top"), b"top").unwrap();
    std::fs::write(from.join("modules/namespace/entry"), b"compiled").unwrap();
    let to = scratch.path().join("to");
    link_files(&from, &to).unwrap();
    assert_eq!(std::fs::read(to.join("top")).unwrap(), b"top");
    assert_eq!(
        std::fs::read(to.join("modules/namespace/entry")).unwrap(),
        b"compiled"
    );
    let untouched = scratch.path().join("untouched");
    link_files(&scratch.path().join("absent"), &untouched).unwrap();
    assert!(!untouched.exists());
}
