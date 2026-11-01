//! Resource diagnostics, each path shown with the source of the nearest listed path.

use super::{HomeLookup, ResourceItem, Snapshots, format_display_path, get_short_path, read};
use indexmap::IndexMap;
use maestro_app::presentation_data::slash_commands::BUILTIN_SLASH_COMMANDS;
use maestro_app::{
    DiagnosticType, ExtensionRunner, Local, ResourceDiagnostic, SourceInfo, SourceScope,
};
use maestro_theme::{Theme, ThemeColor};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet, hash_map::RandomState},
    error::Error,
    hash::BuildHasher,
    rc::Rc,
};

/// Where one listed path came from.
pub(super) enum SourceInfoRef<'a> {
    /// A resource's own provenance record.
    Resource(&'a SourceInfo),
    /// A theme's shared provenance slot.
    Theme(Rc<RefCell<SourceInfo>>),
}

impl SourceInfoRef<'_> {
    /// Runs `read` on the provenance record.
    fn with<R>(&self, read: impl FnOnce(&SourceInfo) -> R) -> R {
        match self {
            Self::Resource(info) => read(info),
            Self::Theme(info) => read(&info.borrow()),
        }
    }
}

/// The provenance of the listed paths, found for a diagnostic path at the path itself
/// or else at its nearest parent ending before a `/`.
///
/// Paths are stored by their literal `/`-separated segments, empty ones included, so a
/// lookup hashes each segment of the diagnostic path once instead of every prefix.
pub(super) struct SourceInfos<'a, S = RandomState> {
    /// Each stored prefix by the prefix before its last `/` and its last segment.
    nodes: HashMap<(usize, &'a str), usize, S>,
    /// The provenance stored at each prefix; the first entry is the empty sequence.
    values: Vec<Option<SourceInfoRef<'a>>>,
}

impl<'a> SourceInfos<'a> {
    /// Indexes the listed extensions, skills, the loader's prompt templates and the
    /// themes loaded from a file, in that order; a later path replaces an earlier one.
    pub(super) fn new(extensions: &'a [ResourceItem<'a>], loaded: &Snapshots<'a>) -> Self {
        let mut index = Self::default();
        for item in extensions {
            if let Some(info) = item.source_info {
                index.insert(item.path, SourceInfoRef::Resource(info));
            }
        }
        for skill in &loaded.skills.skills {
            index.insert(
                &skill.file_path,
                SourceInfoRef::Resource(&skill.source_info),
            );
        }
        for prompt in &loaded.prompts.prompts {
            index.insert(
                &prompt.file_path,
                SourceInfoRef::Resource(&prompt.source_info),
            );
        }
        for theme in &loaded.themes.themes {
            if let (Some(path), Some(info)) = (theme.source_path(), theme.source_info())
                && !path.is_empty()
            {
                index.insert(path, SourceInfoRef::Theme(info));
            }
        }
        index
    }
}

impl Default for SourceInfos<'_> {
    fn default() -> Self {
        Self::with_hasher(RandomState::new())
    }
}

impl<'a, S: BuildHasher> SourceInfos<'a, S> {
    /// An empty index hashing segments with `hasher`.
    pub(super) fn with_hasher(hasher: S) -> Self {
        Self {
            nodes: HashMap::with_hasher(hasher),
            values: vec![None],
        }
    }

    /// Stores `source` for `path`, replacing what the path had.
    pub(super) fn insert(&mut self, path: &'a str, source: SourceInfoRef<'a>) {
        let mut node = 0;
        for segment in path.split('/') {
            let next = self.values.len();
            node = *self.nodes.entry((node, segment)).or_insert(next);
            if node == next {
                self.values.push(None);
            }
        }
        self.values[node] = Some(source);
    }

    /// The provenance of `path` itself, else of its nearest stored parent.
    pub(super) fn find(&self, path: &str) -> Option<&SourceInfoRef<'a>> {
        let mut node = 0;
        let mut parent = None;
        let mut segments = path.split('/').peekable();
        while let Some(segment) = segments.next() {
            let Some(&next) = self.nodes.get(&(node, segment)) else {
                return parent;
            };
            node = next;
            let found = self.values[node].as_ref().or(parent);
            if segments.peek().is_none() {
                return found;
            }
            parent = found;
        }
        parent
    }
}

/// The label of a source and the scope shown after it: local resources by scope,
/// command-line resources as `path`, and any other source as written with its scope.
pub(super) fn get_display_source_info(
    source_info: Option<&SourceInfo>,
) -> (&str, Option<&'static str>) {
    let (source, scope) = source_info.map_or(("local", SourceScope::Project), |info| {
        (info.source.as_str(), info.scope)
    });
    let scope_label = match scope {
        SourceScope::User => "user",
        SourceScope::Project => "project",
        SourceScope::Temporary => "temp",
    };
    let temporary = (scope == SourceScope::Temporary).then_some(scope_label);
    match source {
        "local" if scope == SourceScope::Temporary => ("path", temporary),
        "local" => (scope_label, None),
        "cli" => ("path", temporary),
        _ => (source, Some(scope_label)),
    }
}

/// Shows `path` after its source label, short within its package, or with only its
/// home prefix abbreviated when no source is known.
///
/// # Errors
/// Returns the failure of a path resolution or the home lookup.
pub(super) fn format_path_with_source(
    path: &str,
    source: Option<&SourceInfoRef<'_>>,
    home: &HomeLookup<'_>,
) -> Result<String, Box<dyn Error>> {
    let Some(source) = source else {
        return Ok(format_display_path(path, home)?);
    };
    source.with(|info| {
        let short = get_short_path(path, Some(info), home)?;
        Ok(match get_display_source_info(Some(info)) {
            (label, Some(scope)) => format!("{label} ({scope}) {short}"),
            (label, None) => format!("{label} {short}"),
        })
    })
}

/// Formats collisions grouped by name in first-seen order, each with its first winner
/// and every skipped path, then every other diagnostic in order: its path and indented
/// message, or the message alone.
///
/// # Errors
/// Returns the failure of a path display or of the theme's colours.
pub(super) fn format_diagnostics(
    diagnostics: &[&ResourceDiagnostic],
    source_infos: &SourceInfos<'_>,
    theme: &Theme,
    home: &HomeLookup<'_>,
) -> Result<String, Box<dyn Error>> {
    let mut collisions: IndexMap<&str, (&str, Vec<&str>)> = IndexMap::new();
    let mut others = Vec::new();
    for diagnostic in diagnostics {
        match &diagnostic.collision {
            Some(collision) if diagnostic.r#type == DiagnosticType::Collision => collisions
                .entry(&collision.name)
                .or_insert_with(|| (&collision.winner_path, Vec::new()))
                .1
                .push(&collision.loser_path),
            _ => others.push(*diagnostic),
        }
    }
    let shown = |path: &str| format_path_with_source(path, source_infos.find(path), home);
    let mut lines = Vec::new();
    for (name, (winner, losers)) in collisions {
        lines.push(theme.fg(&ThemeColor::Warning, &format!("  \"{name}\" collision:"))?);
        let kept = theme.fg(&ThemeColor::Success, "✓")?;
        lines.push(theme.fg(&ThemeColor::Dim, &format!("    {kept} {}", shown(winner)?))?);
        for loser in losers {
            let skipped = theme.fg(&ThemeColor::Warning, "✗")?;
            let row = format!("    {skipped} {} (skipped)", shown(loser)?);
            lines.push(theme.fg(&ThemeColor::Dim, &row)?);
        }
    }
    for diagnostic in others {
        let color = if diagnostic.r#type == DiagnosticType::Error {
            ThemeColor::Error
        } else {
            ThemeColor::Warning
        };
        if let Some(path) = diagnostic.path.as_deref().filter(|path| !path.is_empty()) {
            lines.push(theme.fg(&color, &format!("  {}", shown(path)?))?);
            lines.push(theme.fg(&color, &format!("    {}", diagnostic.message))?);
        } else {
            lines.push(theme.fg(&color, &format!("  {}", diagnostic.message))?);
        }
    }
    Ok(lines.join("\n"))
}

/// Warns about each registered extension command named like an ordinary interactive
/// command: it is skipped in completion, or reached by its numbered name.
pub(super) fn get_built_in_command_conflict_diagnostics(
    runner: &ExtensionRunner<Local>,
) -> Vec<ResourceDiagnostic> {
    let builtins: HashSet<_> = BUILTIN_SLASH_COMMANDS
        .iter()
        .map(|command| command.name.as_str())
        .collect();
    runner
        .get_registered_commands()
        .into_iter()
        .filter(|resolved| builtins.contains(resolved.command.name.as_str()))
        .map(|resolved| {
            let name = &resolved.command.name;
            let invocation = &resolved.invocation_name;
            let reached = if invocation == name {
                "Skipping in autocomplete.".to_owned()
            } else {
                format!("Available as '/{invocation}'.")
            };
            ResourceDiagnostic {
                r#type: DiagnosticType::Warning,
                message: format!(
                    "Extension command '/{name}' conflicts with built-in interactive command. {reached}"
                ),
                path: Some(read(&resolved.command.source_info).path.clone()),
                collision: None,
            }
        })
        .collect()
}
