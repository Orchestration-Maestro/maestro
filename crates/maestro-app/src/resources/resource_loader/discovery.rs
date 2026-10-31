//! Reload orchestration: settings refresh, source resolution, the provenance index and
//! path merging.
use super::{DefaultEventBus, DefaultResourceLoader, ResourceExtensionPath, ResourceKind};
use indexmap::IndexMap;
use maestro_packages::{PackageManager, ResolvedPaths, ResolvedResource};
use maestro_path::{SEP, expand_tilde, join};
use maestro_resources::{
    PathMetadata, SourceOrigin, SourceScope, canonicalize_path,
    paths::{home_directory, resolve},
};
use std::{collections::HashSet, io, path::Path, rc::Rc};

/// Paths in insertion order; lookups walk a path's separator-bounded prefixes, so their
/// cost follows the path's depth rather than the number of entries.
pub(super) struct PathIndex<T>(IndexMap<String, T>);

impl<T> Default for PathIndex<T> {
    fn default() -> Self {
        Self(IndexMap::new())
    }
}

impl<T> PathIndex<T> {
    /// Adds `path` unless it is already present.
    fn insert_first(&mut self, path: &str, value: T) {
        if !self.0.contains_key(path) {
            self.0.insert(path.to_owned(), value);
        }
    }
    /// Sets `path`, keeping an existing entry's position.
    pub(super) fn insert(&mut self, path: String, value: T) {
        self.0.insert(path, value);
    }
    /// The entry for exactly `path`.
    pub(super) fn get(&self, path: &str) -> Option<&T> {
        self.0.get(path)
    }
    /// The first-inserted entry that is `path` or a separator-bounded prefix of it.
    pub(super) fn nearest(&self, path: &str) -> Option<&T> {
        path.match_indices(SEP)
            .map(|(end, _)| &path[..end])
            .filter(|prefix| !prefix.is_empty())
            .chain(std::iter::once(path))
            .filter_map(|key| self.0.get_full(key))
            .min_by_key(|(index, _, _)| *index)
            .map(|(_, _, value)| value)
    }
}

/// Enabled paths of one reload, before additional paths are merged.
struct EnabledPaths {
    /// Configured and automatic extensions.
    extensions: Vec<String>,
    /// Configured and automatic skills, directories mapped to their `SKILL.md`.
    skills: Vec<String>,
    /// Configured and automatic prompt templates.
    prompts: Vec<String>,
    /// Configured and automatic themes.
    themes: Vec<String>,
}

/// Indexes every resource of `resources` (first entry per path) and returns the enabled ones.
fn enabled(
    index: &mut super::PathIndex<PathMetadata>,
    resources: Vec<ResolvedResource>,
) -> Vec<ResolvedResource> {
    for resource in &resources {
        index.insert_first(&resource.path, resource.metadata.clone());
    }
    resources.into_iter().filter(|r| r.enabled).collect()
}

/// The paths of the enabled resources of `resources`, indexing all of them.
fn enabled_paths(
    index: &mut super::PathIndex<PathMetadata>,
    resources: Vec<ResolvedResource>,
) -> Vec<String> {
    enabled(index, resources)
        .into_iter()
        .map(|r| r.path)
        .collect()
}

/// The `SKILL.md` of an automatic or package skill directory, indexed with the
/// directory's provenance; otherwise the path itself.
fn map_skill_path(
    index: &mut super::PathIndex<PathMetadata>,
    resource: ResolvedResource,
) -> String {
    let automatic =
        resource.metadata.source == "auto" || resource.metadata.origin == SourceOrigin::Package;
    if !automatic || !Path::new(&resource.path).is_dir() {
        return resource.path;
    }
    let skill_file = join(&[&resource.path, "SKILL.md"]);
    if !Path::new(&skill_file).exists() {
        return resource.path;
    }
    index.insert_first(&skill_file, resource.metadata);
    skill_file
}

/// Builds the provenance index in reference order and returns the enabled paths:
/// configured and automatic resources first, then explicit extension sources.
fn index_resources(
    index: &mut super::PathIndex<PathMetadata>,
    resolved: ResolvedPaths,
    explicit: ResolvedPaths,
) -> (EnabledPaths, EnabledPaths) {
    let extensions = enabled_paths(index, resolved.extensions);
    let skill_resources = enabled(index, resolved.skills);
    let prompts = enabled_paths(index, resolved.prompts);
    let themes = enabled_paths(index, resolved.themes);
    let skills = skill_resources
        .into_iter()
        .map(|resource| map_skill_path(index, resource))
        .collect();
    let cli = PathMetadata {
        source: "cli".into(),
        scope: SourceScope::Temporary,
        origin: SourceOrigin::TopLevel,
        base_dir: None,
    };
    for resource in explicit.extensions.iter().chain(&explicit.skills) {
        index.insert_first(&resource.path, cli.clone());
    }
    let explicit = EnabledPaths {
        extensions: enabled_paths(index, explicit.extensions),
        skills: enabled_paths(index, explicit.skills),
        prompts: enabled_paths(index, explicit.prompts),
        themes: enabled_paths(index, explicit.themes),
    };
    let configured = EnabledPaths {
        extensions,
        skills,
        prompts,
        themes,
    };
    (configured, explicit)
}

impl<E: DefaultEventBus> DefaultResourceLoader<E> {
    /// Discovers every resource again, in reference order.
    pub(super) async fn reload_resources(&self) -> io::Result<()> {
        let flushed = self.settings_manager.borrow().flush();
        flushed.await;
        self.settings_manager.borrow_mut().reload_from_storage();
        let (mut resolved, base_extensions) = self.resolve_packages().await?;
        let mut explicit = self
            .package_manager
            .resolve_extension_sources(
                &self.options.additional_extension_paths,
                Some(SourceScope::Temporary),
            )
            .await?;
        let contributions = self.merge_contributions(&mut explicit, &mut resolved)?;
        let (configured, explicit) = {
            let mut state = self.state.borrow_mut();
            state.contributions = Rc::new(crate::contributions::load_contributions(&contributions));
            state.skill_sources.sources = super::PathIndex::default();
            state.prompt_sources.sources = super::PathIndex::default();
            state.theme_sources.sources = super::PathIndex::default();
            state.metadata = super::PathIndex::default();
            index_resources(&mut state.metadata, resolved, explicit)
        };
        let extension_paths = self.select(
            ResourceKind::Extensions,
            explicit.extensions,
            configured.extensions,
            &[],
        )?;
        self.load_extension_resources(extension_paths, base_extensions)
            .await?;
        let options = &self.options;
        for (collection, kind, explicit, configured, additional) in [
            (
                super::Collection::Skills,
                ResourceKind::Skills,
                explicit.skills,
                configured.skills,
                &options.additional_skill_paths,
            ),
            (
                super::Collection::Prompts,
                ResourceKind::PromptTemplates,
                explicit.prompts,
                configured.prompts,
                &options.additional_prompt_template_paths,
            ),
            (
                super::Collection::Themes,
                ResourceKind::Themes,
                explicit.themes,
                configured.themes,
                &options.additional_theme_paths,
            ),
        ] {
            let paths = self.select(kind, explicit, configured, additional)?;
            self.state.borrow_mut().contributed_mut(collection).paths = paths;
            self.update(collection, additional)?;
        }
        self.load_prompt_files()
    }

    /// Merge enabled contribution files canonically, retaining the first source's metadata.
    fn merge_contributions(
        &self,
        explicit: &mut ResolvedPaths,
        configured: &mut ResolvedPaths,
    ) -> io::Result<Vec<ResolvedResource>> {
        let mut resources = IndexMap::new();
        for mut resource in explicit
            .contributions
            .drain(..)
            .chain(configured.contributions.drain(..))
            .filter(|r| r.enabled)
        {
            resource.path = self.resolve_resource_path(&resource.path)?;
            resources.entry(resource.path.clone()).or_insert(resource);
        }
        let paths: Vec<_> = resources.keys().cloned().collect();
        Ok(self
            .merge_paths(&paths, [])?
            .into_iter()
            .filter_map(|path| resources.swap_remove(&path))
            .collect())
    }

    /// Explicit paths, then configured ones unless `kind` is disabled, then `additional`,
    /// merged. Extensions with their discovery disabled keep the explicit paths unmerged.
    fn select(
        &self,
        kind: ResourceKind,
        mut explicit: Vec<String>,
        configured: Vec<String>,
        additional: &[String],
    ) -> io::Result<Vec<String>> {
        if self.is_disabled(kind) {
            if kind == ResourceKind::Extensions {
                return Ok(explicit);
            }
        } else {
            explicit.extend(configured);
        }
        self.merge_paths(&explicit, additional.iter().map(String::as_str))
    }

    /// Resolves the location of each entry.
    pub(super) fn resolve_entries(
        &self,
        entries: Vec<ResourceExtensionPath>,
    ) -> io::Result<Vec<ResourceExtensionPath>> {
        entries
            .into_iter()
            .map(|entry| {
                Ok(ResourceExtensionPath {
                    path: self.resolve_resource_path(&entry.path)?,
                    metadata: entry.metadata,
                })
            })
            .collect()
    }

    /// Resolves `primary` then `additional`, keeping the first path per real path.
    pub(super) fn merge_paths<'a>(
        &self,
        primary: &'a [String],
        additional: impl IntoIterator<Item = &'a str>,
    ) -> io::Result<Vec<String>> {
        let mut seen = HashSet::new();
        let mut merged = Vec::new();
        for path in primary.iter().map(String::as_str).chain(additional) {
            let resolved = self.resolve_resource_path(path)?;
            if seen.insert(canonicalize_path(&resolved)) {
                merged.push(resolved);
            }
        }
        Ok(merged)
    }

    /// Trims `path`, expands a leading `~` to the home directory and resolves it against
    /// the loader's working directory.
    pub(super) fn resolve_resource_path(&self, path: &str) -> io::Result<String> {
        let expanded = expand_resource_path(path, home_directory)?;
        resolve(&[&self.cwd, &expanded])
    }
}

/// Trim `path` and expand a leading `~` below the home directory read through `home`.
///
/// # Errors
/// Returns the error of `home` when the path needs it.
fn expand_resource_path(
    path: &str,
    home: impl FnOnce() -> io::Result<String>,
) -> io::Result<String> {
    expand_tilde(path.trim_matches(maestro_text::is_whitespace_scalar), home)
}

#[cfg(test)]
mod tests {
    use super::expand_resource_path;

    /// Each resource form returns the reference's result for the six supplied homes, spelled in
    /// the host's path flavor.
    #[test]
    fn resource_paths_follow_the_reference_table() {
        let rows = if cfg!(windows) {
            [
                ("~", r"C:\h", r"C:\h"),
                ("~/", r"C:\h", r"C:\h"),
                ("~/x", r"C:\h", r"C:\h\x"),
                ("~x", r"C:\h", "~x"),
                ("~", r"C:\h\.", r"C:\h\."),
                ("~", "", ""),
                ("~/x", "", "x"),
            ]
        } else {
            [
                ("~", "/h", "/h"),
                ("~/", "/h", "/h"),
                ("~/x", "/h", "/h/x"),
                ("~x", "/h", "~x"),
                ("~", "/h/.", "/h/."),
                ("~", "", ""),
                ("~/x", "", "x"),
            ]
        };
        for (written, home, expected) in rows {
            assert_eq!(
                expand_resource_path(written, || Ok(home.to_owned())).unwrap(),
                expected,
                "{written:?} with home {home:?}"
            );
        }
    }

    /// The form is trimmed before it is expanded, as the reference does.
    #[test]
    fn padded_forms_expand_as_trimmed() {
        let (home, expected) = if cfg!(windows) {
            (r"C:\h", r"C:\h\x")
        } else {
            ("/h", "/h/x")
        };
        assert_eq!(
            expand_resource_path("  ~/x ", || Ok(home.to_owned())).unwrap(),
            expected
        );
    }
}
