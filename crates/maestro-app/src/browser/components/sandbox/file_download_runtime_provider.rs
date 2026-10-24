//! Returned-file records preserving raw host values.
use crate::browser::components::sandboxed_iframe::SandboxFile;
use crate::utils::lock::{read, write};

/// A returned file whose content and metadata retain their original values.
pub type DownloadableFile<V = serde_json::Value> = SandboxFile<V, V>;

use super::providers::{RuntimeProviderValue, SandboxRespond, SandboxRuntimeProvider};
use indexmap::IndexMap;
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::sync::{Arc, RwLock};

/// Live file collection shared with callers.
type FileCollection<V> = Arc<RwLock<Vec<DownloadableFile<V>>>>;

/// Collects returned files without triggering a host download.
pub struct FileDownloadRuntimeProvider<V: RuntimeProviderValue = serde_json::Value> {
    /// Replaceable slot retaining the current caller-visible collection.
    files: RwLock<FileCollection<V>>,
}
impl<V: RuntimeProviderValue> Default for FileDownloadRuntimeProvider<V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<V: RuntimeProviderValue> FileDownloadRuntimeProvider<V> {
    /// Create an empty returned-file collection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: RwLock::default(),
        }
    }
    /// Retain the live collection, including caller edits.
    #[must_use]
    pub fn get_files(&self) -> Arc<RwLock<Vec<DownloadableFile<V>>>> {
        Arc::clone(&read(&self.files))
    }
    /// Replace the current collection without clearing earlier aliases.
    pub fn reset(&self) {
        let old = std::mem::take(&mut *write(&self.files));
        drop(old);
    }
    /// Collect and acknowledge immediately, leaving only settlement to the future.
    fn collect_message(
        &self,
        message: &V,
        respond: &SandboxRespond<V>,
    ) -> Result<(), DiagnosticErrorInfo> {
        if message.message_type()?.as_deref() == Some("file-returned") {
            let file = message.project_download()?;
            let files = Arc::clone(&read(&self.files));
            write(&files).push(file);
            respond(V::success_response()?)?;
        }
        Ok(())
    }
}
impl<V: RuntimeProviderValue> SandboxRuntimeProvider<V> for FileDownloadRuntimeProvider<V> {
    fn handle_message(
        &self,
        message: V,
        respond: SandboxRespond<V>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(self.collect_message(&message, &respond)))
    }

    fn get_data(&self) -> Result<IndexMap<String, V>, DiagnosticErrorInfo> {
        Ok(IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(include_str!("file_download_runtime_provider/runtime.js").into())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok("returnDownloadableFile(filename, content, mimeType?) - Create downloadable file for user (one-time download, not accessible later)".into())
    }
}
