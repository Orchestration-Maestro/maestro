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
