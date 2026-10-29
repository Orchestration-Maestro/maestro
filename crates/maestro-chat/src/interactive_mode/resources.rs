//! The loaded-resource listing appended to the transcript at startup.

use super::{
    InteractiveMode,
    startup::{ExpandableText, TextFactory},
};
use crate::components::shorten_home_path;
use diagnostics::SourceInfos;
use indexmap::IndexMap;
use maestro_app::{
    ContextFile, LoadPromptsResult, LoadSkillsResult, LoadThemesResult, PromptTemplate,
    ReadonlySessionManager, Skill, SourceInfo, SourceScope, home_directory, is_whitespace_scalar,
};
use maestro_path::{Cwd, is_absolute, relative};
use maestro_theme::{Collator, Theme, ThemeColor};
use maestro_tui::Spacer;
use std::{
    collections::HashMap,
    error::Error,
    io,
    rc::Rc,
    sync::{PoisonError, RwLock, RwLockReadGuard},
};

mod diagnostics;
mod labels;
mod listing;
#[cfg(test)]
#[cfg(unix)]
mod tests;

/// Looks up the home directory for a path display that abbreviates it.
type HomeLookup<'a> = dyn Fn() -> io::Result<String> + 'a;

/// One listed resource: its path and, when known, where it came from.
pub(super) struct ResourceItem<'a> {
    /// The resource file.
    pub(super) path: &'a str,
    /// Where the resource came from.
    pub(super) source_info: Option<&'a SourceInfo>,
}

/// What one listing call lists.
#[derive(Default, Clone, Copy)]
pub(super) struct ShowLoadedResourcesOptions<'a> {
    /// The extensions listed in place of the loaded ones, even when empty.
    pub(super) extensions: Option<&'a [ResourceItem<'a>]>,
    /// Lists the sections even on a quiet startup.
    pub(super) force: bool,
    /// Shows the diagnostics even on a quiet startup that lists nothing.
    pub(super) show_diagnostics_when_quiet: bool,
}

/// The loader's results one listing reads once.
struct Snapshots<'a> {
    /// The skills and their diagnostics.
    skills: &'a LoadSkillsResult,
    /// The loader's own prompt templates and their diagnostics.
    prompts: &'a LoadPromptsResult,
    /// The themes and their diagnostics.
    themes: &'a LoadThemesResult,
}

/// The scope a resource is listed under, in listing order.
#[derive(Clone, Copy)]
enum Scope {
    /// Project configuration.
    Project,
    /// User configuration.
    User,
    /// Command-line and temporary resources.
    Path,
}

impl Scope {
    /// The scope's heading.
    fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::User => "user",
            Self::Path => "path",
        }
    }
}

/// The resources of one scope: plain paths, then package resources by package source.
struct ScopeGroup<'a> {
    /// The scope.
    scope: Scope,
    /// The resources outside packages.
    paths: Vec<&'a ResourceItem<'a>>,
    /// The package resources by package source, in first-seen order.
    packages: IndexMap<&'a str, Vec<&'a ResourceItem<'a>>>,
}

/// The scope of a resource: command-line and temporary resources list under path, and
/// a resource of unknown origin under project.
fn get_scope_group(source_info: Option<&SourceInfo>) -> Scope {
    match source_info {
        Some(info) if info.source == "cli" || info.scope == SourceScope::Temporary => Scope::Path,
        Some(info) if info.scope == SourceScope::User => Scope::User,
        _ => Scope::Project,
    }
}

/// Whether a resource came from an npm or Git package.
fn is_package_source(source_info: Option<&SourceInfo>) -> bool {
    source_info
        .is_some_and(|info| info.source.starts_with("npm:") || info.source.starts_with("git:"))
}

/// Groups resources by scope in one pass, keeping the nonempty scopes in listing order.
fn build_scope_groups<'a>(
    items: impl IntoIterator<Item = &'a ResourceItem<'a>>,
) -> Vec<ScopeGroup<'a>> {
    let mut groups = [Scope::Project, Scope::User, Scope::Path].map(|scope| ScopeGroup {
        scope,
        paths: Vec::new(),
        packages: IndexMap::new(),
    });
    for item in items {
        let group = &mut groups[get_scope_group(item.source_info) as usize];
        match item.source_info {
            Some(info) if is_package_source(item.source_info) => {
                group.packages.entry(&info.source).or_default().push(item);
            }
            _ => group.paths.push(item),
        }
    }
    groups
        .into_iter()
        .filter(|group| !group.paths.is_empty() || !group.packages.is_empty())
        .collect()
}

/// Reads `lock`, whose writers never leave it inconsistent.
fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

/// Displays `path` with the home prefix abbreviated, looking the home directory up now.
///
/// # Errors
/// Returns the failure of the home lookup.
fn format_display_path(path: &str, home: &HomeLookup<'_>) -> io::Result<String> {
    Ok(shorten_home_path(path, Some(&home()?)))
}

/// Displays a context file relative to `cwd` when it lies inside it. A file elsewhere, or
/// one whose relative path starts with `..`, keeps its absolute path with the home prefix
/// abbreviated. Both operands resolve lexically.
///
/// # Errors
/// Returns the working-directory error of a resolution that needed it, or the failure
/// of the home lookup.
fn format_context_path(path: &str, cwd: &str, home: &HomeLookup<'_>) -> io::Result<String> {
    let cwd = maestro_app::resolve(&[cwd])?;
    let absolute = maestro_app::resolve(&[&cwd, path])?;
    let relative = relative(
        &cwd,
        &absolute,
        &Cwd {
            current: &cwd,
            drive_directories: &[],
        },
    );
    if relative.is_empty() {
        Ok(".".to_owned())
    } else if !relative.starts_with("..") && !is_absolute(&relative) {
        Ok(relative)
    } else {
        format_display_path(&absolute, home)
    }
}

/// Displays a package resource relative to its package root when it lies below it,
/// otherwise the path inside a recognised npm or Git package directory, otherwise the
/// path with the home prefix abbreviated.
///
/// # Errors
/// Returns the working-directory error of a resolution that needed it, or the failure
/// of the home lookup.
fn get_short_path(
    path: &str,
    source_info: Option<&SourceInfo>,
    home: &HomeLookup<'_>,
) -> Result<String, Box<dyn Error>> {
    if let Some(base_dir) = source_info
        .filter(|_| is_package_source(source_info))
        .and_then(|info| info.base_dir.as_deref())
        .filter(|base_dir| !base_dir.is_empty())
    {
        let base_dir = maestro_app::resolve(&[base_dir])?;
        let short = relative(
            &base_dir,
            &maestro_app::resolve(&[path])?,
            &Cwd {
                current: &base_dir,
                drive_directories: &[],
            },
        );
        if !short.is_empty() && short != "." && !short.starts_with("..") && !is_absolute(&short) {
            return Ok(short.replace('\\', "/"));
        }
    }
    let source = source_info.map_or("", |info| info.source.as_str());
    for (prefix, pattern, group) in [
        ("npm:", r"node_modules/(@?[^/]+(?:/[^/]+)?)/(.*)", 2),
        ("git:", r"git/[^/]+/[^/]+/(.*)", 1),
    ] {
        if source.starts_with(prefix)
            && let Some(range) = regress::Regex::new(pattern)?
                .find(path)
                .and_then(|found| found.group(group))
        {
            return Ok(path[range].to_owned());
        }
    }
    Ok(format_display_path(path, home)?)
}

/// Copies `items` sorted by path under `collator`, equal paths keeping their order.
fn sorted_by_path<'a>(
    items: &[&'a ResourceItem<'a>],
    collator: &Collator,
) -> Vec<&'a ResourceItem<'a>> {
    let mut items = items.to_vec();
    items.sort_by(|a, b| collator.compare(a.path, b.path));
    items
}

/// Lists each scope, then its paths and its package sources with their paths, each
/// sorted by locale.
///
/// # Errors
/// Returns the failure of a path display or of the theme's colours.
fn format_scope_groups(
    groups: &[ScopeGroup<'_>],
    theme: &Theme,
    collator: &Collator,
    format_path: impl Fn(&ResourceItem<'_>) -> Result<String, Box<dyn Error>>,
    format_package_path: impl Fn(&ResourceItem<'_>) -> Result<String, Box<dyn Error>>,
) -> Result<String, Box<dyn Error>> {
    let mut lines = Vec::new();
    for group in groups {
        lines.push(format!(
            "  {}",
            theme.fg(&ThemeColor::Accent, group.scope.label())?
        ));
        for item in sorted_by_path(&group.paths, collator) {
            lines.push(theme.fg(&ThemeColor::Dim, &format!("    {}", format_path(item)?))?);
        }
        let mut packages: Vec<_> = group.packages.iter().collect();
        packages.sort_by(|(a, _), (b, _)| collator.compare(a, b));
        for (source, items) in packages {
            lines.push(format!("    {}", theme.fg(&ThemeColor::MdLink, source)?));
            for item in sorted_by_path(items, collator) {
                let path = format_package_path(item)?;
                lines.push(theme.fg(&ThemeColor::Dim, &format!("      {path}"))?);
            }
        }
    }
    Ok(lines.join("\n"))
}

/// Lists resources grouped by scope and package, each plain path with the home prefix
/// abbreviated and each package path relative to its package.
///
/// # Errors
/// Returns the failure of a path display or of the theme's colours.
fn format_resource_groups(
    items: &[ResourceItem<'_>],
    theme: &Theme,
    collator: &Collator,
    home: &HomeLookup<'_>,
) -> Result<String, Box<dyn Error>> {
    format_scope_groups(
        &build_scope_groups(items),
        theme,
        collator,
        |item| Ok(format_display_path(item.path, home)?),
        |item| get_short_path(item.path, item.source_info, home),
    )
}

/// Joins the trimmed nonempty labels as one dimmed row, sorted by locale when a
/// collator is supplied and in their supplied order otherwise.
///
/// # Errors
/// Returns the failure of the theme's dim colour.
fn format_compact_list<'a>(
    items: impl Iterator<Item = &'a str>,
    theme: &Theme,
    collator: Option<&Collator>,
) -> Result<String, Box<dyn Error>> {
    let mut labels: Vec<_> = items
        .map(|item| item.trim_matches(is_whitespace_scalar))
        .filter(|item| !item.is_empty())
        .collect();
    if let Some(collator) = collator {
        labels.sort_by(|a, b| collator.compare(a, b));
    }
    Ok(theme.fg(&ThemeColor::Dim, &format!("  {}", labels.join(", ")))?)
}

impl InteractiveMode {
    /// Whether startup text begins expanded: when verbose or when tool output is expanded.
    pub(super) fn get_startup_expansion_state(&self) -> bool {
        self.options.verbose || self.tool_output_expanded.get()
    }

    /// Appends the loaded context files, skills, prompt templates, extensions and themes
    /// as expandable transcript sections, then the resource diagnostics as static text.
    ///
    /// The sections are listed when `force` or verbose is set or startup is not quiet;
    /// the diagnostics then, or when asked for even on a quiet startup.
    ///
    /// # Errors
    /// Returns the failure of the collator, the home lookup, a path resolution or the
    /// theme's colours.
    pub(super) fn show_loaded_resources(
        &self,
        options: ShowLoadedResourcesOptions<'_>,
    ) -> Result<(), Box<dyn Error>> {
        self.list_loaded_resources(options, &home_directory)
    }

    /// Appends the loaded resources as `show_loaded_resources` does, looking the home
    /// directory up through `home`.
    ///
    /// # Errors
    /// Returns the failure of the collator, the home lookup, a path resolution or the
    /// theme's colours.
    fn list_loaded_resources(
        &self,
        options: ShowLoadedResourcesOptions<'_>,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let show_listing = options.force
            || self.options.verbose
            || !session.settings_manager().borrow().get_quiet_startup();
        if !show_listing && !options.show_diagnostics_when_quiet {
            return Ok(());
        }
        let theme = self.themes.theme().get()?;
        let collator = Collator::new(&|key| std::env::var(key).ok())?;
        let loader = session.resource_loader();
        let skills = loader.get_skills();
        let prompts = loader.get_prompts();
        let themes = loader.get_themes();
        let loader_extensions = options
            .extensions
            .is_none()
            .then(|| loader.get_extensions());
        let owners: Vec<_> = loader_extensions
            .iter()
            .flat_map(|result| &result.extensions)
            .map(|owner| read(owner))
            .collect();
        let sources: Vec<_> = owners
            .iter()
            .map(|owner| read(&owner.source_info))
            .collect();
        let items: Vec<_> = owners
            .iter()
            .zip(&sources)
            .map(|(owner, source)| ResourceItem {
                path: &owner.path,
                source_info: Some(source),
            })
            .collect();
        let extensions = options.extensions.unwrap_or(&items);
        let resources = Snapshots {
            skills: &skills,
            prompts: &prompts,
            themes: &themes,
        };
        let source_infos = SourceInfos::new(extensions, &resources);
        if show_listing {
            self.show_context_resources(&loader.get_agents_files(), &theme, home)?;
            self.show_skills(&skills.skills, &theme, &collator, home)?;
            self.show_prompts(&session.prompt_templates().prompts, &theme, &collator, home)?;
            self.show_extensions(extensions, &theme, &collator, home)?;
            self.show_themes(&themes.themes, &theme, &collator, home)?;
        }
        self.show_diagnostics(&resources, &source_infos, &theme, home)
    }

    /// Appends a spacer and the context section: compact paths in discovery order, or
    /// each path on its own row in the spelling the loader supplied, with the home prefix
    /// abbreviated.
    ///
    /// # Errors
    /// Returns the failure of a path resolution, the home lookup or the theme's colours.
    fn show_context_resources(
        &self,
        files: &[ContextFile],
        theme: &Theme,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if files.is_empty() {
            return Ok(());
        }
        let cwd = self
            .runtime_host
            .session()
            .session_manager()
            .borrow()
            .get_cwd()
            .to_owned();
        let compact = files
            .iter()
            .map(|file| format_context_path(&file.path, &cwd, home))
            .collect::<io::Result<Vec<_>>>()?;
        let expanded = files
            .iter()
            .map(|file| {
                let path = format_display_path(&file.path, home)?;
                Ok(theme.fg(&ThemeColor::Dim, &format!("  {path}"))?)
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        self.add_loaded_section(
            "Context",
            format_compact_list(compact.iter().map(String::as_str), theme, None)?,
            expanded.join("\n"),
        )
    }

    /// Appends the skills section: sorted names, or paths grouped by scope and package.
    ///
    /// # Errors
    /// Returns the failure of a path display or of the theme's colours.
    fn show_skills(
        &self,
        skills: &[Skill],
        theme: &Theme,
        collator: &Collator,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if skills.is_empty() {
            return Ok(());
        }
        let items: Vec<_> = skills
            .iter()
            .map(|skill| ResourceItem {
                path: &skill.file_path,
                source_info: Some(&skill.source_info),
            })
            .collect();
        let expanded = format_resource_groups(&items, theme, collator, home)?;
        let names = skills.iter().map(|skill| skill.name.as_str());
        let compact = format_compact_list(names, theme, Some(collator))?;
        self.add_loaded_section("Skills", compact, expanded)
    }

    /// Appends the prompts section: sorted slash names, or the slash name of the last
    /// template at each path, grouped by scope and package.
    ///
    /// # Errors
    /// Returns the failure of the theme's colours.
    fn show_prompts(
        &self,
        prompts: &[PromptTemplate],
        theme: &Theme,
        collator: &Collator,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if prompts.is_empty() {
            return Ok(());
        }
        let items: Vec<_> = prompts
            .iter()
            .map(|prompt| ResourceItem {
                path: &prompt.file_path,
                source_info: Some(&prompt.source_info),
            })
            .collect();
        let by_path: HashMap<_, _> = prompts
            .iter()
            .map(|prompt| (prompt.file_path.as_str(), prompt.name.as_str()))
            .collect();
        let label = |item: &ResourceItem<'_>| {
            Ok(match by_path.get(item.path) {
                Some(name) => format!("/{name}"),
                None => format_display_path(item.path, home)?,
            })
        };
        let expanded =
            format_scope_groups(&build_scope_groups(&items), theme, collator, label, label)?;
        let names: Vec<_> = prompts
            .iter()
            .map(|prompt| format!("/{}", prompt.name))
            .collect();
        let compact = format_compact_list(names.iter().map(String::as_str), theme, Some(collator))?;
        self.add_loaded_section("Prompts", compact, expanded)
    }

    /// Appends one expandable section and a trailing spacer; each rendering pairs the
    /// live theme's heading with its body as prepared now.
    ///
    /// # Errors
    /// Returns the failure of the heading colour when the initial rendering is sampled.
    fn add_loaded_section(
        &self,
        name: &str,
        collapsed: String,
        expanded: String,
    ) -> Result<(), Box<dyn Error>> {
        let factory = |body: String| -> TextFactory {
            let live = self.themes.theme();
            let heading = format!("[{name}]");
            Box::new(move || {
                let heading = live.get()?.fg(&ThemeColor::MdHeading, &heading)?;
                Ok(format!("{heading}\n{body}"))
            })
        };
        self.chat_container.add_child(Rc::new(ExpandableText::new(
            factory(collapsed),
            factory(expanded),
            self.get_startup_expansion_state(),
            0,
            0,
        )?));
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        Ok(())
    }
}
