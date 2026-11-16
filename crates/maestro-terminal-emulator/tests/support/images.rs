use maestro_test_directory::Scratch;

/// Exclusively write a file below the scratch directory, returning its path.
fn fixture_file(scratch: &Scratch, name: impl AsRef<std::path::Path>, data: &[u8]) -> std::path::PathBuf {
    use std::io::Write as _;
    let path = scratch.path().join(name);
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
    file.write_all(data).unwrap();
    path
}
