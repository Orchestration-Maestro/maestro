//! File-backed credential fixtures shared by entry and credential witnesses.
#![cfg(test)]

/// Disposable exclusively created files owned by one test transaction.
pub(crate) struct Files(
    /// Directory owned by this transaction.
    pub(crate) std::path::PathBuf,
);

impl Files {
    /// Create a unique directory without overwriting an existing path.
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "maestro-cloud-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    /// Resolve a fixture path without creating it.
    pub(crate) fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }

    /// Write one new fixture file, preserving any pre-existing file.
    pub(crate) fn write(&self, name: &str, bytes: &[u8]) -> String {
        use std::io::Write;
        let path = self.path(name);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
        path
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

/// File-backed workforce credentials with a controlled endpoint and distinguishable quota.
pub(crate) fn workforce(endpoint: &str, quota: &str) -> serde_json::Value {
    serde_json::json!({"type":"external_account_authorized_user","client_id":"controlled-id","client_secret":"controlled-secret","refresh_token":"controlled-refresh","token_url":endpoint,"quota_project_id":quota})
}
