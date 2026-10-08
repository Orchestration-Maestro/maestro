//! Supplied-path instruction resources with ordered metadata.
#![doc = include_str!("../../../docs/resources/skills.md")]

pub mod diagnostics;
pub use diagnostics::{DiagnosticType, ResourceCollision, ResourceDiagnostic};
pub mod frontmatter;
pub mod paths;
pub mod skills;
pub mod source_info;
pub use frontmatter::{
    FrontmatterError, FrontmatterValue, ParsedFrontmatter, parse_frontmatter, strip_frontmatter,
};
pub use paths::{canonicalize_path, is_local_path};
#[cfg(not(target_arch = "wasm32"))]
pub use skills::NativeResourceOperations;
pub use skills::{
    LoadSkillsFromDirOptions, LoadSkillsOptions, LoadSkillsResult, ResourceEntry, ResourceFileType,
    ResourceOperations, Skill, SkillFrontmatter, format_skills_for_prompt, load_skills,
    load_skills_from_dir,
};
pub use source_info::{
    PathMetadata, SourceInfo, SourceOrigin, SourceScope, SyntheticSourceOptions,
    create_source_info, create_synthetic_source_info,
};
