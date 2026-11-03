//! The prepared native fixture installed as a controlled child.
use std::path::{Path, PathBuf};

/// Install the prepared fixture as `name` below `directory`, selected by `record`.
pub fn install(directory: &Path, name: &str, family: &str, record: &serde_json::Value) -> PathBuf {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .map(PathBuf::from)
        .expect("run just prepare-tests, then run these tests with cargo nextest");
    std::fs::create_dir_all(directory).expect("the fixture directory is creatable");
    let path = directory.join(name);
    maestro_test_process::install_executable(&artifact, &path).expect("install starts");
    select(&path, family, record);
    path
}

/// Rewrite one installed fixture's record, retaining its executable.
pub fn select(path: &Path, family: &str, record: &serde_json::Value) {
    let mut record_path = path.as_os_str().to_owned();
    record_path.push(".fixture");
    std::fs::write(record_path, format!("{family}\n{record}")).expect("the record is writable");
}
