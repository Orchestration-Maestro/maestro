//! Retained sandbox loading and injectable document preparation.
#[cfg(any(target_arch = "wasm32", test))]
mod document;
#[cfg(any(target_arch = "wasm32", test))]
mod execution;
#[cfg(any(target_arch = "wasm32", test))]
mod runtime;
#[cfg(test)]
mod tests;

/// Select the document envelope and online helpers independently.
#[derive(Clone, Copy, Default)]
pub struct PrepareHtmlOptions {
    /// Inject into authored HTML instead of wrapping JavaScript.
    pub is_html_artifact: bool,
    /// Omit parent messaging and navigation interception for downloads.
    pub is_standalone: bool,
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::{BrowserSandboxHost, SandboxIframe};
