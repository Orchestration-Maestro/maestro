//! Styled rows and position status for the borrowed tree.
use super::{FilterMode, TreeList};
use maestro_theme::{LiveTheme, ThemeColor};
use maestro_tui::{TruncateOptions, truncate_to_width, tui::TuiRuntime};
use std::io;
impl TreeList<'_, '_> {
    /// Renders the visible window followed by its position and filter status.
    ///
    /// # Errors
    /// Returns `InvalidData` for reached malformed display fields, or a theme failure.
    pub fn render(
        &self,
        width: usize,
        theme: &LiveTheme,
        runtime: &dyn TuiRuntime,
        styles_enabled: bool,
    ) -> io::Result<Vec<String>> {
        let theme = theme.get().map_err(io::Error::other)?;
        let clip = |line: &str| truncate_to_width(line, width, TruncateOptions::default());
        if self.visible.is_empty() {
            return [
                "  No entries found".to_owned(),
                format!("  (0/0){}", self.status()),
            ]
            .iter()
            .map(|line| {
                Ok(clip(
                    &theme
                        .fg(&ThemeColor::Muted, line)
                        .map_err(io::Error::other)?,
                ))
            })
            .collect();
        }
        let start = self
            .selected
            .saturating_sub(self.max_visible_lines / 2)
            .min(self.visible.len().saturating_sub(self.max_visible_lines));
        let end = (start + self.max_visible_lines).min(self.visible.len());
        let mut lines = Vec::new();
        for index in start..end {
            lines.push(clip(&self.row(index, &theme, runtime, styles_enabled)?));
        }
        lines.push(clip(
            &theme
                .fg(
                    &ThemeColor::Muted,
                    &format!(
                        "  ({}/{}){}",
                        self.selected + 1,
                        self.visible.len(),
                        self.status()
                    ),
                )
                .map_err(io::Error::other)?,
        ));
        Ok(lines)
    }
    /// Compose one row before final terminal-cell clipping.
    fn row(
        &self,
        index: usize,
        theme: &maestro_theme::Theme,
        runtime: &dyn TuiRuntime,
        styles: bool,
    ) -> io::Result<String> {
        let node = self.visible[index];
        let selected = index == self.selected;
        let fg = |key, text: &str| theme.fg(&key, text).map_err(io::Error::other);
        let cursor = if selected {
            fg(ThemeColor::Accent, "› ")?
        } else {
            "  ".to_owned()
        };
        let active = node.entry["id"]
            .as_str()
            .is_some_and(|id| self.active_path.contains(id));
        let marker = if active {
            fg(ThemeColor::Accent, "• ")?
        } else {
            String::new()
        };
        let label = node
            .label
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|label| fg(ThemeColor::Warning, &format!("[{label}] ")))
            .transpose()?
            .unwrap_or_default();
        let content =
            if node.entry["type"] == "message" && node.entry["message"]["role"] == "toolResult" {
                fg(
                    ThemeColor::Muted,
                    &self.tool_result(&node.entry["message"], runtime)?,
                )?
            } else {
                super::entry_text::entry(node.entry, theme, styles)?
            };
        let content = maestro_theme::Theme::bold(&content, selected && styles);
        let line =
            cursor + &fg(ThemeColor::Dim, &self.prefix(index))? + &marker + &label + &content;
        if selected {
            theme
                .bg(&maestro_theme::ThemeBg::SelectedBg, &line)
                .map_err(io::Error::other)
        } else {
            Ok(line)
        }
    }
    /// Three-cell levels place continuing gutters before branch connectors.
    fn prefix(&self, index: usize) -> String {
        let row = &self.structure.rows[index];
        let indent = row
            .indent
            .saturating_sub(usize::from(self.structure.multiple_roots));
        let connector = row.show_connector && !row.virtual_child;
        let foldable = self.visible[index].entry["id"]
            .as_str()
            .is_some_and(|id| self.structure.is_foldable(id));
        (0..indent)
            .map(|level| {
                if let Some(gutter) = row.gutters.iter().find(|g| g.position == level) {
                    ["   ", "│  "][usize::from(gutter.show)]
                } else if connector && level + 1 == indent {
                    match (row.is_last, foldable) {
                        (true, true) => "└⊟ ",
                        (true, false) => "└─ ",
                        (false, true) => "├⊟ ",
                        (false, false) => "├─ ",
                    }
                } else {
                    "   "
                }
            })
            .collect()
    }
    /// Leaves this uncached view unchanged.
    pub fn invalidate(&self) {}
    /// The badge for the nondefault category selection.
    fn status(&self) -> &'static str {
        match self.mode {
            FilterMode::Default => "",
            FilterMode::NoTools => " [no-tools]",
            FilterMode::UserOnly => " [user]",
            FilterMode::LabeledOnly => " [labeled]",
            FilterMode::All => " [all]",
        }
    }
}
