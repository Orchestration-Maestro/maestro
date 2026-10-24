//! The extension and theme sections and the diagnostics of the loaded-resource listing.

use super::diagnostics::{
    SourceInfos, format_diagnostics, get_built_in_command_conflict_diagnostics,
};
use super::labels::{
    format_extension_display_path, get_compact_extension_labels, get_compact_path_label,
};
use super::{
    HomeLookup, InteractiveMode, ResourceItem, Snapshots, build_scope_groups, format_compact_list,
    format_resource_groups, format_scope_groups, get_short_path, read,
};
use maestro_app::{DiagnosticType, ResourceDiagnostic};
use maestro_theme::{Collator, Theme, ThemeColor};
use maestro_tui::{Spacer, Text};
use std::{error::Error, rc::Rc};

impl InteractiveMode {
    /// Appends the extensions section: sorted compact labels, or display paths without
    /// their index file grouped by scope and package.
    ///
    /// # Errors
    /// Returns the failure of a path display or of the theme's colours.
    pub(super) fn show_extensions(
        &self,
        items: &[ResourceItem<'_>],
        theme: &Theme,
        collator: &Collator,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if items.is_empty() {
            return Ok(());
        }
        let expanded = format_scope_groups(
            &build_scope_groups(items),
            theme,
            collator,
            |item| Ok(format_extension_display_path(item.path, home)?),
            |item| {
                let short = get_short_path(item.path, item.source_info, home)?;
                Ok(format_extension_display_path(&short, home)?)
            },
        )?;
        let labels = get_compact_extension_labels(items, home)?;
        let compact =
            format_compact_list(labels.iter().map(String::as_str), theme, Some(collator))?;
        self.add_loaded_section("Extensions", compact, expanded)
    }

    /// Appends the themes section for the themes loaded from a file: sorted names, a
    /// nameless theme by its compact path, or paths grouped by scope and package.
    ///
    /// # Errors
    /// Returns the failure of a path display or of the theme's colours.
    pub(super) fn show_themes(
        &self,
        themes: &[Rc<Theme>],
        theme: &Theme,
        collator: &Collator,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let located: Vec<_> = themes
            .iter()
            .filter_map(|loaded| {
                let path = loaded.source_path().filter(|path| !path.is_empty())?;
                Some((loaded, path, loaded.source_info()))
            })
            .collect();
        if located.is_empty() {
            return Ok(());
        }
        let sources: Vec<_> = located
            .iter()
            .map(|(_, _, source)| source.as_ref().map(|source| source.borrow()))
            .collect();
        let items: Vec<_> = located
            .iter()
            .zip(&sources)
            .map(|((_, path, _), source)| ResourceItem {
                path,
                source_info: source.as_deref(),
            })
            .collect();
        let expanded = format_resource_groups(&items, theme, collator, home)?;
        let names = located
            .iter()
            .zip(&items)
            .map(|((loaded, ..), item)| match loaded.name() {
                Some(name) => Ok(name.to_owned()),
                None => get_compact_path_label(item, home),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let compact = format_compact_list(names.iter().map(String::as_str), theme, Some(collator))?;
        self.add_loaded_section("Themes", compact, expanded)
    }

    /// Appends the skill, prompt, extension and theme diagnostics, each nonempty group
    /// as static text under its warning heading followed by a spacer. Extension issues
    /// are the load failures, the command diagnostics published before this listing,
    /// the built-in command conflicts and the shortcut diagnostics.
    ///
    /// # Errors
    /// Returns the failure of a path display or of the theme's colours.
    pub(super) fn show_diagnostics(
        &self,
        loaded: &Snapshots<'_>,
        source_infos: &SourceInfos<'_>,
        theme: &Theme,
        home: &HomeLookup<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let section = |name: &str, diagnostics: &[&ResourceDiagnostic]| {
            if diagnostics.is_empty() {
                return Ok::<_, Box<dyn Error>>(());
            }
            let lines = format_diagnostics(diagnostics, source_infos, theme, home)?;
            let heading = theme.fg(&ThemeColor::Warning, &format!("[{name}]"))?;
            let text = Text::new(format!("{heading}\n{lines}"), 0, 0, None);
            self.chat_container.add_child(Rc::new(text));
            self.chat_container.add_child(Rc::new(Spacer::new(1)));
            Ok(())
        };
        section(
            "Skill conflicts",
            &loaded.skills.diagnostics.iter().collect::<Vec<_>>(),
        )?;
        section(
            "Prompt conflicts",
            &loaded.prompts.diagnostics.iter().collect::<Vec<_>>(),
        )?;
        let session = self.runtime_host.session();
        let errors: Vec<_> = session
            .resource_loader()
            .get_extensions()
            .errors
            .iter()
            .map(|error| ResourceDiagnostic {
                r#type: DiagnosticType::Error,
                message: error.error.clone(),
                path: Some(error.path.clone()),
                collision: None,
            })
            .collect();
        let runner = session.extension_runner();
        let commands = runner.get_command_diagnostics();
        let conflicts = get_built_in_command_conflict_diagnostics(&runner);
        let shortcuts = runner.get_shortcut_diagnostics();
        let (commands, shortcuts) = (read(&commands), read(&shortcuts));
        let issues: Vec<_> = errors
            .iter()
            .chain(commands.iter())
            .chain(&conflicts)
            .chain(shortcuts.iter())
            .collect();
        section("Extension issues", &issues)?;
        section(
            "Theme conflicts",
            &loaded.themes.diagnostics.iter().collect::<Vec<_>>(),
        )
    }
}
