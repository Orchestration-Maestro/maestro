//! Skill, prompt template and theme results: loading, overrides, provenance and name
//! deduplication.
use super::{
    Collection, DefaultEventBus, DefaultResourceLoader, LoadPromptsResult, LoadThemesResult,
    PathIndex, ResourceKind, ResourceOverride,
};
use maestro_path::{SEP, dirname, join};
use maestro_resources::{
    DiagnosticType, LoadPromptTemplatesOptions, LoadSkillsOptions, LoadSkillsResult, PathMetadata,
    PromptTemplate, ResourceCollision, ResourceDiagnostic, SourceInfo, SourceOrigin, SourceScope,
    create_source_info, is_local_path, load_prompt_templates, load_skills,
    paths::{CONFIG_DIR_NAME, resolve},
};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io,
    path::Path,
    rc::Rc,
};

/// The roots whose resources default to user or project scope.
const DEFAULT_ROOTS: [&str; 4] = ["skills", "prompts", "themes", "extensions"];

/// Applies `override_result` to `base` when present.
pub(super) fn apply<T>(override_result: Option<&ResourceOverride<T>>, base: T) -> T {
    match override_result {
        Some(replace) => replace(base),
        None => base,
    }
}

/// Prompt templates with the first per name kept and a collision for each later one.
fn dedupe_prompts(prompts: Vec<PromptTemplate>) -> LoadPromptsResult {
    let mut result = LoadPromptsResult::default();
    let mut winners: HashMap<String, String> = HashMap::new();
    for prompt in prompts {
        if let Some(winner) = winners.get(&prompt.name) {
            result.diagnostics.push(collision(
                format!("name \"/{}\" collision", prompt.name),
                "prompt",
                &prompt.name,
                winner.clone(),
                prompt.file_path.clone(),
            ));
        } else {
            winners.insert(prompt.name.clone(), prompt.file_path.clone());
            result.prompts.push(prompt);
        }
    }
    result
}

/// Themes with the first per name (`unnamed` when absent) kept and a collision for each
/// later one.
pub(super) fn dedupe_themes(loaded: LoadThemesResult) -> LoadThemesResult {
    let LoadThemesResult {
        themes,
        mut diagnostics,
    } = loaded;
    let mut kept = Vec::new();
    let mut winners: HashMap<String, Option<String>> = HashMap::new();
    for theme in themes {
        let name = theme.name().unwrap_or("unnamed").to_owned();
        let path = theme.source_path().map(str::to_owned);
        if let Some(winner) = winners.get(&name) {
            let builtin =
                |path: &Option<String>| path.clone().unwrap_or_else(|| "<builtin>".into());
            let mut diagnostic = collision(
                format!("name \"{name}\" collision"),
                "theme",
                &name,
                builtin(winner),
                builtin(&path),
            );
            diagnostic.path = path;
            diagnostics.push(diagnostic);
        } else {
            winners.insert(name, path);
            kept.push(theme);
        }
    }
    LoadThemesResult {
        themes: kept,
        diagnostics,
    }
}

/// A name collision diagnostic at the loser's path.
fn collision(
    message: String,
    resource_type: &str,
    name: &str,
    winner_path: String,
    loser_path: String,
) -> ResourceDiagnostic {
    ResourceDiagnostic {
        r#type: DiagnosticType::Collision,
        message,
        path: Some(loser_path.clone()),
        collision: Some(ResourceCollision {
            resource_type: resource_type.into(),
            name: name.into(),
            winner_path,
            loser_path,
            winner_source: None,
            loser_source: None,
        }),
    }
}

/// Whether the resolved `target` is `root` or below it.
fn is_under(target: &str, root: &str) -> io::Result<bool> {
    let root = resolve(&[root])?;
    if target == root {
        return Ok(true);
    }
    Ok(if root.ends_with(SEP) {
        target.starts_with(&root)
    } else {
        target
            .strip_prefix(&root)
            .is_some_and(|rest| rest.starts_with(SEP))
    })
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Rebuilds `collection` from its last paths and adds a missing-path error for each
    /// of `extra` that does not exist.
    pub(super) fn update(&self, collection: Collection, extra: &[String]) -> io::Result<()> {
        match collection {
            Collection::Skills => self.update_skills(extra),
            Collection::Prompts => self.update_prompts(extra),
            Collection::Themes => self.update_themes(extra),
        }
    }

    /// Loads, overrides and attributes skills, then stores them.
    fn update_skills(&self, extra: &[String]) -> io::Result<()> {
        let loaded = {
            let state = self.state.borrow();
            let paths = &state.skill_sources.paths;
            if self.is_disabled(ResourceKind::Skills) && paths.is_empty() {
                LoadSkillsResult::default()
            } else {
                load_skills(LoadSkillsOptions {
                    cwd: &self.cwd,
                    // Every path is already resolved, so no tilde expansion reads home.
                    home: "",
                    agent_dir: &self.agent_dir,
                    config_dir_name: CONFIG_DIR_NAME,
                    skill_paths: paths,
                    include_defaults: false,
                })?
            }
        };
        let mut result = apply(self.options.skills_override.as_ref(), loaded);
        for skill in &mut result.skills {
            if let Some(source_info) = self.attribute(&skill.file_path, Collection::Skills)? {
                skill.source_info = source_info;
            }
        }
        self.report_missing(
            &mut result.diagnostics,
            extra,
            "Skill path does not exist",
            true,
        )?;
        self.state.borrow_mut().skills = Rc::new(result);
        Ok(())
    }

    /// Loads, deduplicates, overrides and attributes prompt templates, then stores them.
    fn update_prompts(&self, extra: &[String]) -> io::Result<()> {
        let loaded = {
            let state = self.state.borrow();
            let paths = &state.prompt_sources.paths;
            if self.is_disabled(ResourceKind::PromptTemplates) && paths.is_empty() {
                LoadPromptsResult::default()
            } else {
                dedupe_prompts(load_prompt_templates(LoadPromptTemplatesOptions {
                    cwd: &self.cwd,
                    // Every path is already resolved, so no tilde expansion reads home.
                    home: "",
                    agent_dir: &self.agent_dir,
                    config_dir_name: CONFIG_DIR_NAME,
                    prompt_paths: paths,
                    include_defaults: false,
                })?)
            }
        };
        let mut result = apply(self.options.prompts_override.as_ref(), loaded);
        for prompt in &mut result.prompts {
            if let Some(source_info) = self.attribute(&prompt.file_path, Collection::Prompts)? {
                prompt.source_info = source_info;
            }
        }
        self.report_missing(
            &mut result.diagnostics,
            extra,
            "Prompt template path does not exist",
            true,
        )?;
        self.state.borrow_mut().prompts = Rc::new(result);
        Ok(())
    }

    /// Loads, deduplicates, overrides and attributes themes, then stores them.
    fn update_themes(&self, extra: &[String]) -> io::Result<()> {
        let loaded = {
            let state = self.state.borrow();
            let paths = &state.theme_sources.paths;
            if self.is_disabled(ResourceKind::Themes) && paths.is_empty() {
                LoadThemesResult::default()
            } else {
                dedupe_themes(self.load_themes(paths)?)
            }
        };
        let mut result = apply(self.options.themes_override.as_ref(), loaded);
        for theme in &result.themes {
            let Some(source_path) = theme.source_path().filter(|path| !path.is_empty()) else {
                continue;
            };
            let source_info = match self.attribute(source_path, Collection::Themes)? {
                Some(found) => found,
                None if theme.source_info().is_some() => continue,
                None => self.default_source_info(source_path)?,
            };
            theme.set_source_info(Some(Rc::new(RefCell::new(source_info))));
        }
        self.report_missing(
            &mut result.diagnostics,
            extra,
            "Theme path does not exist",
            false,
        )?;
        self.state.borrow_mut().themes = Rc::new(result);
        Ok(())
    }

    /// Provenance of a resource of `collection` from its contributions or the reload index.
    fn attribute(&self, path: &str, collection: Collection) -> io::Result<Option<SourceInfo>> {
        let state = self.state.borrow();
        self.find_source_info(
            path,
            Some(&state.contributed(collection).sources),
            &state.metadata,
        )
    }

    /// Adds `message` for each of `extra` whose resolved path does not exist, unless a
    /// diagnostic already names that spelling; with `local_only`, package sources are skipped.
    fn report_missing(
        &self,
        diagnostics: &mut Vec<ResourceDiagnostic>,
        extra: &[String],
        message: &str,
        local_only: bool,
    ) -> io::Result<()> {
        let mut named: HashSet<String> =
            diagnostics.iter().filter_map(|d| d.path.clone()).collect();
        for path in extra {
            if (local_only && !is_local_path(path))
                || Path::new(&self.resolve_resource_path(path)?).exists()
                || !named.insert(path.clone())
            {
                continue;
            }
            diagnostics.push(ResourceDiagnostic {
                r#type: DiagnosticType::Error,
                message: message.into(),
                path: Some(path.clone()),
                collision: None,
            });
        }
        Ok(())
    }

    /// Provenance for `resource_path`: a `<label>` default, an extension contribution
    /// covering it, or the indexed entry for it or its nearest indexed ancestor.
    pub(super) fn find_source_info(
        &self,
        resource_path: &str,
        contributed: Option<&PathIndex<SourceInfo>>,
        metadata: &PathIndex<PathMetadata>,
    ) -> io::Result<Option<SourceInfo>> {
        if resource_path.is_empty() {
            return Ok(None);
        }
        if resource_path.starts_with('<') {
            return self.default_source_info(resource_path).map(Some);
        }
        let normalized = resolve(&[resource_path])?;
        if let Some(source) = contributed.and_then(|sources| sources.nearest(&normalized)) {
            return Ok(Some(SourceInfo {
                path: resource_path.to_owned(),
                ..source.clone()
            }));
        }
        let found = metadata
            .get(&normalized)
            .or_else(|| metadata.get(resource_path))
            .or_else(|| metadata.nearest(&normalized));
        Ok(found.map(|entry| create_source_info(resource_path.to_owned(), entry.clone())))
    }

    /// Provenance derived from the path alone: a `<label:…>` source, user or project scope
    /// under the default roots, otherwise local and based at the directory itself or the
    /// file's parent.
    pub(super) fn default_source_info(&self, file_path: &str) -> io::Result<SourceInfo> {
        let synthetic = |source: &str, scope, base_dir| SourceInfo {
            path: file_path.to_owned(),
            source: source.into(),
            scope,
            origin: SourceOrigin::TopLevel,
            base_dir,
        };
        if let Some(label) = file_path
            .strip_prefix('<')
            .and_then(|p| p.strip_suffix('>'))
        {
            let source = label.split(':').next().filter(|s| !s.is_empty());
            return Ok(synthetic(
                source.unwrap_or("temporary"),
                SourceScope::Temporary,
                None,
            ));
        }
        let normalized = resolve(&[file_path])?;
        let project = join(&[&self.cwd, CONFIG_DIR_NAME]);
        let roots = [
            (self.agent_dir.as_str(), SourceScope::User),
            (&project, SourceScope::Project),
        ]
        .into_iter()
        .flat_map(|(base, scope)| DEFAULT_ROOTS.map(|name| (join(&[base, name]), scope)));
        for (root, scope) in roots {
            if is_under(&normalized, &root)? {
                return Ok(synthetic("local", scope, Some(root)));
            }
        }
        let base_dir = if Path::new(&normalized).is_dir() {
            normalized
        } else {
            dirname(&normalized)
        };
        Ok(synthetic("local", SourceScope::Temporary, Some(base_dir)))
    }
}
