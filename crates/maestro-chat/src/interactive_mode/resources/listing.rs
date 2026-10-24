//! The theme section of the loaded-resource listing.

use super::{
    HomeLookup, InteractiveMode, ResourceItem, format_compact_list, format_resource_groups,
    labels::get_compact_path_label,
};
use maestro_theme::{Collator, Theme};
use std::{error::Error, rc::Rc};

impl InteractiveMode {
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
}
