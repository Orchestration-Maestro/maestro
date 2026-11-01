//! Application-supplied owners used by native built-in factories.
use crate::types::{ExtensionAPI, ExtensionRuntime, read, write};
use crate::{ExtensionExecution, ToolsManager};
use std::sync::Arc;

/// Existing application tool services, shared rather than rediscovered by extensions.
#[derive(Clone)]
pub struct ToolConstruction {
    /// The application's search-program acquisition owner.
    pub tools_manager: Arc<ToolsManager>,
    /// The application's binary directory for local shell construction.
    pub bin_dir: String,
}
impl<E: ExtensionExecution> ExtensionRuntime<E> {
    /// Installs the application's existing tool services before factories are loaded.
    pub fn set_tool_construction(&self, construction: ToolConstruction) {
        let old = write(&self.tool_construction).replace(construction);
        drop(old);
    }
    /// Reads supplied services without retaining the runtime guard.
    #[must_use]
    pub fn tool_construction(&self) -> Option<ToolConstruction> {
        read(&self.tool_construction).clone()
    }
}
impl<E: ExtensionExecution> ExtensionAPI<E> {
    /// Reads the application's tool services without creating another manager.
    ///
    /// # Errors
    /// Returns the original stale error or an error when services were not supplied.
    pub fn tool_construction(&self) -> Result<ToolConstruction, String> {
        self.owner().map_err(|error| error.to_string())?;
        self.runtime
            .tool_construction()
            .ok_or_else(|| "Extension tool construction is not configured".into())
    }
}
