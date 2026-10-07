//! Skill discovery over caller-supplied filesystem operations.
#![cfg_attr(not(target_arch = "wasm32"), doc = include_str!("../../../docs/skills.md"))]
mod skills;
mod templates {
    pub(crate) mod source_info;
}
pub use skills::diagnostics::{ResourceCollision, ResourceDiagnostic};
pub use skills::frontmatter::{
    FrontmatterValue, ParsedFrontmatter, ResourceError, SkillFrontmatter, parse_frontmatter,
    strip_frontmatter,
};
#[cfg(not(target_arch = "wasm32"))]
pub use skills::operations::NativeResourceOperations;
pub use skills::operations::{Dirent, ResourceOperations, Stats};
pub use skills::paths::{canonicalize_path, is_local_path};
pub use skills::{
    LoadSkillsFromDirOptions, LoadSkillsOptions, LoadSkillsResult, Skill, format_skills_for_prompt,
    load_skills, load_skills_from_dir,
};
pub use templates::source_info::{
    PathMetadata, SourceInfo, SourceOrigin, SourceScope, create_source_info,
    create_synthetic_source_info,
};
