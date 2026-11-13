//! Supplied-path instruction resources, package manifests and extension entry points.
#![doc = include_str!("../../../docs/resources/skills.md")]

pub mod diagnostics;
pub use diagnostics::{DiagnosticType, ResourceCollision, ResourceDiagnostic};
pub mod metadata;
pub mod package_manifest;
pub mod paths;
pub mod prompts;
pub use prompts::{
    LoadPromptTemplatesOptions, PromptTemplate, expand_prompt_template, load_prompt_templates,
    parse_command_args, substitute_args,
};
pub mod provenance;
mod skills;
pub use maestro_interchange::slash_commands::{SlashCommandInfo, SlashCommandSource};
#[doc(inline)]
pub use maestro_path::fs::{current_dir, drive_dirs};
pub use metadata::{
    FrontmatterError, FrontmatterValue, ParsedFrontmatter, parse_frontmatter, parse_yaml,
    strip_frontmatter,
};
pub use paths::{canonicalize_path, is_local_path};
pub use provenance::{
    PathMetadata, SourceInfo, SourceOrigin, SourceScope, SyntheticSourceOptions,
    create_source_info, create_synthetic_source_info,
};
pub use skills::{
    LoadSkillsFromDirOptions, LoadSkillsOptions, LoadSkillsResult, ResourceEntry, ResourceFileType,
    Skill, SkillFrontmatter, format_skills_for_prompt, load_skills, load_skills_from_dir, metadata,
    observed_type, read_entries, read_text, real_path, symlink_metadata,
};
