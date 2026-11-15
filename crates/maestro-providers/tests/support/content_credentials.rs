//! File-backed credential fixtures shared by entry and credential witnesses.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Disposable exclusively created files owned by one test transaction.
pub(crate) struct Files(
    /// Directory owned by this transaction.
    pub(crate) maestro_test_directory::Scratch,
);

impl Files {
    /// Create a unique directory without overwriting an existing path.
    pub(crate) fn new() -> Self {
        Self(maestro_test_directory::Scratch::new().unwrap())
    }

    /// Resolve a fixture path without creating it.
    pub(crate) fn path(&self, name: &str) -> String {
        self.0.path().join(name).to_str().unwrap().to_owned()
    }
}

/// File-backed workforce credentials with a controlled endpoint and distinguishable quota.
pub(crate) fn workforce(endpoint: &str, quota: &str) -> serde_json::Value {
    serde_json::json!({"type":"external_account_authorized_user","client_id":"controlled-id","client_secret":"controlled-secret","refresh_token":"controlled-refresh","token_url":endpoint,"quota_project_id":quota})
}
