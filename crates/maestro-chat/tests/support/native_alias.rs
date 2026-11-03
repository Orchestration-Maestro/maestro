//! Installed native fixture aliases with their typed record.
use std::path::{Path, PathBuf};

/// Install the prepared fixture as `name` in `directory`, selecting `family`'s `record`.
///
/// The record is written before the alias exists, so a launched program always finds it.
#[cfg(unix)]
pub(crate) fn install(
    directory: &Path,
    name: &str,
    family: &str,
    record: &serde_json::Value,
) -> PathBuf {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("nextest native-fixture setup supplies the prepared artifact");
    let entry = directory.join(name);
    let mut path = entry.as_os_str().to_os_string();
    path.push(".fixture");
    std::fs::write(path, format!("{family}\n{record}")).expect("the record is writable");
    maestro_test_process::install_executable(Path::new(&artifact), &entry)
        .expect("the prepared artifact installs");
    entry
}
