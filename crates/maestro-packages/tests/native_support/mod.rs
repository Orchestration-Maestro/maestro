//! Native file fixtures shared by package behavior tests.
#![cfg(test)]

/// Creates a file fixture and its parent directory.
pub fn write(path: &str, text: &str) {
    std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
