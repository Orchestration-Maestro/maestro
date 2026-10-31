//! Skill discovery over the native file system.

mod discovery;
mod files;
mod prompt;
pub use discovery::observed_type;
pub use prompt::format_skills_for_prompt;
mod real_path;
pub use real_path::real_path;
mod validation;
use crate::{
    ResourceDiagnostic, SourceScope, SyntheticSourceOptions, create_synthetic_source_info,
    parse_frontmatter,
    paths::{is_under_path, resolve, resolve_explicit_path},
};
pub use files::{
    ResourceEntry, ResourceFileType, current_directory, drive_directories, metadata, read_entries,
    read_text, symlink_metadata,
};
use maestro_path::{basename, dirname, join};
use std::{io, path::Path};
pub use validation::SkillFrontmatter;

pub use maestro_request::skills::Skill;
/// Ordered discoveries and their diagnostics.
#[derive(Debug, Default)]
pub struct LoadSkillsResult {
    /// Retained skills in discovery order.
    pub skills: Vec<Skill>,
    /// Returned diagnostics in validation order.
    pub diagnostics: Vec<ResourceDiagnostic>,
}
/// A supplied directory and its source label.
#[derive(Clone, Copy)]
pub struct LoadSkillsFromDirOptions<'a> {
    /// The caller's working directory.
    ///
    /// It is the first operand of each resolution that makes an entry's path
    /// relative to `dir`; the scanned paths are read as given.
    pub cwd: &'a str,
    /// Directory to scan.
    pub dir: &'a str,
    /// Provenance label.
    pub source: &'a str,
}
/// Load instruction resources from the native file system.
///
/// A directory that does not exist yields no skills. Each entry's path is made
/// relative to `dir` with `cwd` as the first operand of both resolutions, except
/// that `dir` itself is empty without resolving; hidden, `node_modules` and
/// broken-link entries are skipped before they are located. The process
/// directories are read at each resolution that `cwd` and `dir` leave open,
/// and only then. A directory whose entries cannot be located
/// because that read fails ends with the skills gathered before the failure, so
/// loading a directory never fails.
#[must_use]
pub fn load_skills_from_dir(options: LoadSkillsFromDirOptions<'_>) -> LoadSkillsResult {
    discovery::scan(options.dir, options.cwd, options.source)
}
/// Load one file, retaining its native failure cause as a warning.
fn load_file(path: &str, source: &str) -> LoadSkillsResult {
    let loaded = read_text(Path::new(path))
        .map_err(|e| e.to_string())
        .and_then(|s| parse_frontmatter(&s).map_err(|e| e.to_string()))
        .and_then(|parsed| {
            validation::project_fields(&parsed.frontmatter).map_err(|e| e.to_string())
        });
    match loaded {
        Ok(fields) => validated_skill(path, source, fields),
        Err(cause) => LoadSkillsResult {
            skills: Vec::new(),
            diagnostics: vec![ResourceDiagnostic::warning(path, cause)],
        },
    }
}
/// Validate supported fields before deciding whether the file is retained.
fn validated_skill(path: &str, source: &str, fields: SkillFrontmatter) -> LoadSkillsResult {
    let base_dir = dirname(path);
    let parent = basename(&base_dir, None);
    let name = fields
        .name
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| parent.clone());
    let description = fields.description.unwrap_or_default();
    let diagnostics = validation::description_warnings(&description)
        .into_iter()
        .chain(validation::name_warnings(&name))
        .map(|message| ResourceDiagnostic::warning(path, message))
        .collect();
    let mut result = LoadSkillsResult {
        skills: Vec::new(),
        diagnostics,
    };
    if !description
        .trim_matches(crate::frontmatter::text_whitespace)
        .is_empty()
    {
        let (scope, label) = match source {
            "user" => (Some(SourceScope::User), "local"),
            "project" => (Some(SourceScope::Project), "local"),
            "path" => (None, "local"),
            _ => (None, source),
        };
        let source_info = create_synthetic_source_info(
            path.into(),
            SyntheticSourceOptions {
                source: label.into(),
                scope,
                origin: None,
                base_dir: Some(base_dir.clone()),
            },
        );
        result.skills.push(Skill {
            name,
            description,
            file_path: path.into(),
            base_dir,
            source_info,
            disable_model_invocation: fields.disable_model_invocation,
        });
    }
    result
}

/// Supplied roots and explicitly requested skill paths.
#[derive(Clone, Copy)]
pub struct LoadSkillsOptions<'a> {
    /// Working directory for relative explicit paths and project defaults.
    ///
    /// It is the first operand of each of those resolutions; the process
    /// directories complete what a relative `cwd` leaves open.
    pub cwd: &'a str,
    /// Caller-resolved home directory.
    pub home: &'a str,
    /// Caller-resolved user configuration directory.
    pub agent_dir: &'a str,
    /// Project configuration directory name.
    pub config_dir_name: &'a str,
    /// Explicit files or scan directories in precedence order.
    pub skill_paths: &'a [String],
    /// Whether user and project defaults precede explicit paths.
    pub include_defaults: bool,
}
/// Load defaults before explicit paths, retaining first names and canonical files.
///
/// The process directories are read at each resolution whose operands leave a
/// part open, and only then.
///
/// With defaults disabled, an existing explicit path under the user or project
/// skills root takes that scope; when resolving the user root needs the
/// unreadable process working directory, that path gets a warning instead.
///
/// # Errors
/// Returns the working-directory error when resolving the project
/// configuration directory, or a relative explicit path, against `cwd` leaves
/// the path unanchored and the process working directory cannot be read.
pub fn load_skills(options: LoadSkillsOptions<'_>) -> io::Result<LoadSkillsResult> {
    load_skills_in(options)
}
/// Load defaults and explicit paths.
///
/// The caller's working directory anchors the project and relative explicit paths.
fn load_skills_in(options: LoadSkillsOptions<'_>) -> io::Result<LoadSkillsResult> {
    let mut result = LoadSkillsResult::default();
    let mut collisions = Vec::new();
    let mut paths = std::collections::HashSet::new();
    let user_skills = join(&[options.agent_dir, "skills"]);
    let project_skills = resolve(&[options.cwd, options.config_dir_name, "skills"])?;
    if options.include_defaults {
        for (path, source) in [(&user_skills, "user"), (&project_skills, "project")] {
            add_skills(
                &mut result,
                &mut collisions,
                &mut paths,
                discovery::scan(path, options.cwd, source),
            );
        }
    }
    let source_of = |path: &str| {
        for (root, label) in [(&user_skills, "user"), (&project_skills, "project")] {
            if !options.include_defaults && is_under_path(path, root)? {
                return Ok(label);
            }
        }
        Ok("path")
    };
    for path in options.skill_paths {
        let path = normalize_skill_path(path, &options)?;
        let loaded = explicit_path(&path, options.cwd, &source_of);
        add_skills(&mut result, &mut collisions, &mut paths, loaded);
    }
    result.diagnostics.extend(collisions);
    Ok(result)
}

/// Resolve an explicit skill path through the shared instruction-path resolver.
fn normalize_skill_path(path: &str, options: &LoadSkillsOptions<'_>) -> io::Result<String> {
    resolve_explicit_path(path, options.cwd, options.home)
}
/// Merge discoveries, retaining the first name and delaying collision messages.
fn add_skills(
    result: &mut LoadSkillsResult,
    collisions: &mut Vec<ResourceDiagnostic>,
    paths: &mut std::collections::HashSet<String>,
    loaded: LoadSkillsResult,
) {
    result.diagnostics.extend(loaded.diagnostics);
    for skill in loaded.skills {
        let real_path = crate::canonicalize_path(&skill.file_path);
        if paths.contains(&real_path) {
            continue;
        }
        if let Some(winner) = result
            .skills
            .iter()
            .find(|winner| winner.name == skill.name)
        {
            collisions.push(ResourceDiagnostic {
                r#type: crate::DiagnosticType::Collision,
                message: format!("name \"{}\" collision", skill.name),
                path: Some(skill.file_path.clone()),
                collision: Some(crate::ResourceCollision {
                    resource_type: "skill".into(),
                    name: skill.name,
                    winner_path: winner.file_path.clone(),
                    loser_path: skill.file_path,
                    winner_source: None,
                    loser_source: None,
                }),
            });
        } else {
            paths.insert(real_path);
            result.skills.push(skill);
        }
    }
}
/// Dispatch a supplied path, keeping missing, wrong-kind and native errors distinct.
///
/// A path that exists is classified by `source_of` after its metadata is read.
/// A classification that fails becomes that path's warning.
fn explicit_path(
    path: &str,
    cwd: &str,
    source_of: &dyn Fn(&str) -> io::Result<&'static str>,
) -> LoadSkillsResult {
    let error = if Path::new(path).exists() {
        let classified =
            metadata(Path::new(path)).and_then(|kind| source_of(path).map(|source| (kind, source)));
        match classified {
            Ok((ResourceFileType::Directory, source)) => {
                return discovery::scan(path, cwd, source);
            }
            Ok((ResourceFileType::File, source))
                if path
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| extension == "md") =>
            {
                return load_file(path, source);
            }
            Ok(_) => "skill path is not a markdown file".into(),
            Err(cause) => cause.to_string(),
        }
    } else {
        "skill path does not exist".into()
    };
    LoadSkillsResult {
        skills: Vec::new(),
        diagnostics: vec![ResourceDiagnostic::warning(path, error)],
    }
}
#[cfg(test)]
mod tests;
