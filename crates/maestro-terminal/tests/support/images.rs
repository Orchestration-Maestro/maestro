/// Per-process fixture identity allocation; no environment state is changed.
static NEXT_FILE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// An exclusively created directory that owns its fixture cleanup.
struct Scratch(std::path::PathBuf);
impl Scratch {
    /// Create one isolated fixture directory under the observed temporary root.
    fn new() -> Self {
        Self::in_dir(&std::env::temp_dir())
    }
    /// Create one fixture directory below a supplied parent.
    fn in_dir(parent: &std::path::Path) -> Self {
        let id = NEXT_FILE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = parent.join(format!("maestro-image-{}-{id}",std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
    /// Exclusively write a file below this directory.
    fn file(&self, name: impl AsRef<std::path::Path>, data: &[u8]) -> std::path::PathBuf {
        use std::io::Write as _;
        let path = self.0.join(name);
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        file.write_all(data).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { std::fs::remove_dir_all(&self.0).unwrap(); }
}
