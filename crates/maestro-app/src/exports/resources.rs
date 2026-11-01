//! Existing resources owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::{
    SourceOrigin, SourceScope,
    paths::{home_directory, is_local_path, resolve},
};

pub use maestro_resources::{
    ResourceEntry, ResourceFileType, canonicalize_path, current_directory, drive_directories,
};

#[cfg(not(target_arch = "wasm32"))]
pub use crate::resources::resource_loader::{
    DefaultEventBus, DefaultResourceLoader, DefaultResourceLoaderOptions, LoadPromptsResult,
    LoadThemesResult, ResourceCollision, ResourceDiagnostic, ResourceExtensionPath,
    ResourceExtensionPaths, ResourceKind, ResourceLoader, ResourceLoaderOptions, ResourceOverride,
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
pub use maestro_resources::{DiagnosticType, LoadSkillsResult};

pub use maestro_resources::{
    LoadSkillsFromDirOptions, PromptTemplate, Skill, SkillFrontmatter, SourceInfo,
    create_synthetic_source_info, format_skills_for_prompt, load_skills, load_skills_from_dir,
    parse_frontmatter, strip_frontmatter,
};
