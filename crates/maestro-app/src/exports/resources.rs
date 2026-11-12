//! Existing resources owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use crate::resources::loading::{
    DefaultEventBus, DefaultResourceLoader, DefaultResourceLoaderOptions, LoadPromptsResult,
    LoadThemesResult, ResourceCollision, ResourceDiagnostic, ResourceExtensionPath,
    ResourceExtensionPaths, ResourceKind, ResourceLoader, ResourceLoaderOptions, ResourceOverride,
    load_project_context_files,
};
#[cfg(not(target_arch = "wasm32"))]
pub use crate::runtime::session_dependencies::{
    AgentSessionRuntimeDiagnostic, AgentSessionRuntimeDiagnosticType, AgentSessionServices,
    CreateAgentSessionServicesOptions, create_agent_session_services,
};
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_extensions::LoadExtensionsResult;
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::LoadSkillsResult;

pub use maestro_resources::{
    LoadSkillsFromDirOptions, PromptTemplate, Skill, SkillFrontmatter, SourceInfo,
    create_synthetic_source_info, format_skills_for_prompt, load_skills, load_skills_from_dir,
    parse_frontmatter, strip_frontmatter,
};
