//! The loaded-resource listing appended to the transcript at startup.

use super::{
    InteractiveMode,
    startup::{ExpandableText, TextFactory},
};
use crate::components::shorten_home_path as format_display_path;
use maestro_app::{ContextFile, ReadonlySessionManager, home_directory, is_whitespace_scalar};
use maestro_path::{Cwd, is_absolute, relative};
use maestro_theme::{Theme, ThemeColor};
use maestro_tui::Spacer;
use std::{error::Error, io, rc::Rc};

#[cfg(test)]
#[cfg(unix)]
mod tests;

/// Displays a context file relative to `cwd` when it lies inside it, otherwise as an
/// absolute path with the home prefix abbreviated; both operands resolve lexically.
///
/// # Errors
/// Returns the working-directory error of a resolution that needed it.
fn format_context_path(path: &str, cwd: &str, home: Option<&str>) -> io::Result<String> {
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
        Ok(format_display_path(&absolute, home))
    }
}

/// Joins the trimmed nonempty labels, in their supplied order, as one dimmed row.
///
/// # Errors
/// Returns the failure of the theme's dim colour.
fn format_compact_list<'a>(
    items: impl Iterator<Item = &'a str>,
    theme: &Theme,
) -> Result<String, Box<dyn Error>> {
    let labels: Vec<_> = items
        .map(|item| item.trim_matches(is_whitespace_scalar))
        .filter(|item| !item.is_empty())
        .collect();
    Ok(theme.fg(&ThemeColor::Dim, &format!("  {}", labels.join(", ")))?)
}

impl InteractiveMode {
    /// Whether startup text begins expanded: when verbose or when tool output is expanded.
    pub(super) fn get_startup_expansion_state(&self) -> bool {
        self.options.verbose || self.tool_output_expanded.get()
    }

    /// Appends the loaded context files as an expandable transcript section, unless
    /// startup is quiet without verbose and `force` is not set.
    ///
    /// # Errors
    /// Returns the failure of the home lookup, a path resolution or the theme's colours.
    pub(super) fn show_loaded_resources(&self, force: bool) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        if !force
            && !self.options.verbose
            && session.settings_manager().borrow().get_quiet_startup()
        {
            return Ok(());
        }
        let theme = self.themes.theme().get()?;
        let home = home_directory()?;
        let files = session.resource_loader().get_agents_files();
        self.show_context_resources(&files, &theme, Some(&home))
    }

    /// Appends a spacer and the context section: compact paths in discovery order, or
    /// each full path on its own row.
    ///
    /// # Errors
    /// Returns the failure of a path resolution or the theme's colours.
    fn show_context_resources(
        &self,
        files: &[ContextFile],
        theme: &Theme,
        home: Option<&str>,
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
                theme.fg(
                    &ThemeColor::Dim,
                    &format!("  {}", format_display_path(&file.path, home)),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.chat_container.add_child(Rc::new(Spacer::new(1)));
        self.add_loaded_section(
            "Context",
            format_compact_list(compact.iter().map(String::as_str), theme)?,
            expanded.join("\n"),
        )
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
