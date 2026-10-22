//! Existing resources owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::{SourceOrigin, SourceScope};

pub use maestro_resources::{
    ResourceEntry, ResourceFileType, canonicalize_path, current_directory, drive_directories,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::resources::resource_loader::{
    DefaultEventBus, DefaultResourceLoader, DefaultResourceLoaderOptions, LoadPromptsResult,
    LoadThemesResult, ResourceCollision, ResourceDiagnostic, ResourceExtensionPaths, ResourceKind,
    ResourceLoader, ResourceLoaderOptions, ResourceOverride, ResourcePath,
    load_project_context_files,
};
#[cfg(not(target_arch = "wasm32"))]
pub use crate::runtime::agent_session_services::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, AgentSessionServices,
    CreateAgentSessionServicesOptions, create_agent_session_services,
};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::{
    ExtensionLoadError, LoadExtensionsResult, ModuleLoader, types::FlagValue,
};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::LoadSkillsResult;
