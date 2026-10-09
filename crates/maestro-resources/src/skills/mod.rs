//! Skill discovery through replaceable filesystem operations.

mod operations;
mod prompt;
pub use prompt::format_skills_for_prompt;
mod discovery;
#[cfg(not(target_arch = "wasm32"))]
mod real_path;
mod validation;
use crate::{
    ResourceDiagnostic, SourceScope, SyntheticSourceOptions, create_synthetic_source_info,
    parse_frontmatter,
};
use maestro_path::{SEP, basename, dirname, is_absolute, join};
#[cfg(not(target_arch = "wasm32"))]
pub use operations::NativeResourceOperations;
pub(crate) use operations::ProcessContext;
pub use operations::{ResourceEntry, ResourceFileType, ResourceOperations};
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
    /// relative to `dir`; the adapter reads the scanned paths as given.
    pub cwd: &'a str,
    /// Directory to scan.
    pub dir: &'a str,
    /// Provenance label.
    pub source: &'a str,
}
/// Load instruction resources using the supplied filesystem observations.
///
/// A directory that does not exist yields no skills. Each entry's path is made
/// relative to `dir` with `cwd` as the first operand of both resolutions, except
/// that `dir` itself is empty without resolving; hidden, `node_modules` and
/// broken-link entries are skipped before they are located. The adapter's
/// process directories are asked for at the first resolution that `cwd` and
/// `dir` leave open, and only then. A directory whose entries cannot be located
/// because that read fails ends with the skills gathered before the failure, so
/// loading a directory never fails.
pub fn load_skills_from_dir(
    options: LoadSkillsFromDirOptions<'_>,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let ctx = ProcessContext::new(operations);
    discovery::scan(options.dir, options.cwd, &ctx, options.source, operations)
}
/// Load one file, retaining its native failure cause as a warning.
fn load_file(path: &str, source: &str, operations: &dyn ResourceOperations) -> LoadSkillsResult {
    let loaded = operations
        .read_file(Path::new(path))
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
        .chain(validation::name_warnings(&name, &parent))
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
    /// directories the adapter reports complete what a relative `cwd` leaves open.
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
/// The adapter's process directories are asked for at most once, at the first
/// resolution whose operands leave a part open; its `canonicalize` is separate.
///
/// With defaults disabled, an existing explicit path under the user or project
/// skills root takes that scope; when resolving the user root needs the
/// unreadable process working directory, that path gets a warning instead.
///
/// # Errors
/// Returns the adapter's working-directory error kind when resolving the
/// project configuration directory, or a relative explicit path, against `cwd`
/// leaves the path unanchored and the adapter cannot read the process working
/// directory.
pub fn load_skills(
    options: LoadSkillsOptions<'_>,
    operations: &dyn ResourceOperations,
) -> io::Result<LoadSkillsResult> {
    load_skills_in(options, &ProcessContext::new(operations), operations)
}
/// Load defaults and explicit paths.
///
/// The caller's working directory anchors the project and relative explicit paths.
fn load_skills_in(
    options: LoadSkillsOptions<'_>,
    ctx: &ProcessContext<'_>,
    operations: &dyn ResourceOperations,
) -> io::Result<LoadSkillsResult> {
    let mut result = LoadSkillsResult::default();
    let mut collisions = Vec::new();
    let mut paths = std::collections::HashSet::new();
    let user_skills = join(&[options.agent_dir, "skills"]);
    let project_skills = ctx.resolve(&[options.cwd, options.config_dir_name, "skills"])?;
    if options.include_defaults {
        for (path, source) in [(&user_skills, "user"), (&project_skills, "project")] {
            add_skills(
                &mut result,
                &mut collisions,
                &mut paths,
                operations,
                discovery::scan(path, options.cwd, ctx, source, operations),
            );
        }
    }
    let source_of = |path: &str| {
        for (root, label) in [(&user_skills, "user"), (&project_skills, "project")] {
            if !options.include_defaults && is_under_path(path, root, ctx)? {
                return Ok(label);
            }
        }
        Ok("path")
    };
    for path in options.skill_paths {
        let path = path.trim_matches(crate::frontmatter::text_whitespace);
        let path = if let Some(suffix) = path.strip_prefix('~') {
            join(&[options.home, suffix])
        } else if is_absolute(path) {
            path.to_owned()
        } else {
            ctx.resolve(&[options.cwd, path])?
        };
        let loaded = explicit_path(&path, (options.cwd, ctx), &source_of, operations);
        add_skills(&mut result, &mut collisions, &mut paths, operations, loaded);
    }
    result.diagnostics.extend(collisions);
    Ok(result)
}
/// Compare preserved target spelling against a root resolved from the process
/// directories, on a separator boundary.
fn is_under_path(target: &str, root: &str, ctx: &ProcessContext<'_>) -> io::Result<bool> {
    let root = ctx.resolve(&[root])?;
    Ok(target == root || target.starts_with(&format!("{}{SEP}", root.trim_end_matches(SEP))))
}
/// Merge discoveries, retaining the first name and delaying collision messages.
fn add_skills(
    result: &mut LoadSkillsResult,
    collisions: &mut Vec<ResourceDiagnostic>,
    paths: &mut std::collections::HashSet<String>,
    operations: &dyn ResourceOperations,
    loaded: LoadSkillsResult,
) {
    result.diagnostics.extend(loaded.diagnostics);
    for skill in loaded.skills {
        let real_path = crate::canonicalize_path(&skill.file_path, operations);
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
    (cwd, ctx): (&str, &ProcessContext<'_>),
    source_of: &dyn Fn(&str) -> io::Result<&'static str>,
    operations: &dyn ResourceOperations,
) -> LoadSkillsResult {
    let error = if operations.exists(Path::new(path)) {
        let classified = operations
            .metadata(Path::new(path))
            .and_then(|kind| source_of(path).map(|source| (kind, source)));
        match classified {
            Ok((ResourceFileType::Directory, source)) => {
                return discovery::scan(path, cwd, ctx, source, operations);
            }
            Ok((ResourceFileType::File, source))
                if path
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| extension == "md") =>
            {
                return load_file(path, source, operations);
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
