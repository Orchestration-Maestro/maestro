//! Native file fixtures shared by package behavior tests.
#![cfg(test)]
use std::path::{Path, PathBuf};

/// Join an authored `/`-separated relative path onto a native root, one component at a time.
pub fn under(root: impl AsRef<Path>, relative: &str) -> PathBuf {
    let mut path = root.as_ref().to_path_buf();
    path.extend(relative.split('/').filter(|part| !part.is_empty()));
    path
}

/// Creates a file fixture and its parent directory.
pub fn write(path: impl AsRef<Path>, text: &str) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Install the prepared native fixture as `name` below `directory`, selected by `record`.
#[cfg(unix)]
pub fn command(directory: &Path, name: &str, record: &serde_json::Value) -> PathBuf {
    std::fs::create_dir_all(directory).unwrap();
    let path = directory.join(name);
    maestro_test_process::install_command(&path, &record.to_string())
        .expect("the prepared native fixture installs");
    path
}

/// The `output` stand-in record: raw streams, an exit status and no signal.
#[cfg(unix)]
pub fn output(stdout: &[u8], stderr: &[u8], code: u8) -> serde_json::Value {
    serde_json::json!({
        "scenario": "output",
        "stdout": stdout,
        "stderr": stderr,
        "code": code,
    })
}
