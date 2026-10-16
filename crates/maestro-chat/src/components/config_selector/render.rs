//! Live header and resource rows through the existing theme/text owners.
use super::{
    ConfigSelectorOptions,
    resource_list::{FlatEntry, ResourceList},
};
use maestro_theme::{Theme, ThemeColor, ThemeError};
use maestro_tui::{Component, TruncateOptions, expand_tabs, truncate_to_width, visible_width};
use std::{error::Error, rc::Rc};

/// Current colors and bold policy for the two header rows.
pub(super) struct ConfigSelectorHeader(pub Rc<ConfigSelectorOptions>);
/// Clips a displayed row, expanding tab bytes through the toolkit owner.
fn clip(text: &str, width: usize, ellipsis: &str) -> String {
    truncate_to_width(
        &expand_tabs(text),
        width,
        TruncateOptions {
            ellipsis,
            ..Default::default()
        },
    )
}
/// Samples an authored foreground role without suppressing missing colors.
fn paint(
    options: &ConfigSelectorOptions,
    color: &ThemeColor,
    text: &str,
) -> Result<String, ThemeError> {
    options.theme.get()?.fg(color, text)
}
impl Component for ConfigSelectorHeader {
    /// Legacy rendering emits no rows when live header styling fails.
    fn render(&self, width: usize) -> Vec<String> {
        self.try_render(width).unwrap_or_default()
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn Error>> {
        let title = Theme::bold("Resource Configuration", (self.0.styles_enabled)());
        let sep = paint(&self.0, &ThemeColor::Muted, " · ")?;
        let hint = maestro_app::raw_key_hint("space", "toggle", &self.0.theme)?
            + &sep
            + &maestro_app::raw_key_hint("esc", "close", &self.0.theme)?;
        let spacing = width
            .saturating_sub(visible_width(&title) + visible_width(&hint))
            .max(1);
        Ok(vec![
            clip(&format!("{title}{}{hint}", " ".repeat(spacing)), width, ""),
            paint(&self.0, &ThemeColor::Muted, "Type to filter resources")?,
        ])
    }
}
impl ResourceList {
    /// Paints a flat entry after releasing any live-item borrow before callbacks.
    fn row(&self, entry: &FlatEntry, selected: bool, width: usize) -> Result<String, ThemeError> {
        let text = match entry {
            FlatEntry::Group(label) => format!(
                "  {}",
                paint(
                    &self.options,
                    &ThemeColor::Accent,
                    &Theme::bold(label, (self.options.styles_enabled)())
                )?
            ),
            FlatEntry::Subgroup(label) => {
                format!("    {}", paint(&self.options, &ThemeColor::Muted, label)?)
            }
            FlatEntry::Item(item) => {
                let (enabled, name) = {
                    let item = item.borrow();
                    (item.enabled, item.display_name.clone())
                };
                let cursor = if selected { "> " } else { "  " };
                let checkbox = if enabled {
                    paint(&self.options, &ThemeColor::Success, "[x]")?
                } else {
                    paint(&self.options, &ThemeColor::Dim, "[ ]")?
                };
                let name = if selected {
                    Theme::bold(&name, (self.options.styles_enabled)())
                } else {
                    name
                };
                format!("{cursor}    {checkbox} {name}")
            }
        };
        Ok(clip(
            &text,
            width,
            if matches!(entry, FlatEntry::Item(_)) {
                "..."
            } else {
                ""
            },
        ))
    }
    /// Renders search, centered flat window and resource-only scroll counts.
    pub(super) fn render_rows(&self, width: usize) -> Result<Vec<String>, Box<dyn Error>> {
        let mut lines = self.search.try_render(width)?;
        lines.push(String::new());
        let count = self.filtered.borrow().len();
        if count == 0 {
            lines.push(paint(
                &self.options,
                &ThemeColor::Muted,
                "  No resources found",
            )?);
            return Ok(lines);
        }
        let selected = self.selected.get();
        let window = super::super::list_window(selected, count, 15);
        for index in window.start..window.end {
            let entry = self.filtered.borrow().get(index).cloned();
            if let Some(entry) = entry {
                lines.push(self.row(&entry, index == self.selected.get(), width)?);
            }
        }
        if window.start > 0 || window.end < self.filtered.borrow().len() {
            let (current, count) = {
                let entries = self.filtered.borrow();
                let item = |entry: &&FlatEntry| matches!(entry, FlatEntry::Item(_));
                (
                    entries
                        .iter()
                        .take(self.selected.get())
                        .filter(item)
                        .count()
                        + 1,
                    entries.iter().filter(item).count(),
                )
            };
            lines.push(paint(
                &self.options,
                &ThemeColor::Dim,
                &format!("  ({current}/{count})"),
            )?);
        }
        Ok(lines)
    }
}
