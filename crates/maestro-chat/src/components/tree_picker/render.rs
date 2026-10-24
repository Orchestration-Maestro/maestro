//! Styled rows and position status for the retained tree.
use super::{FilterMode, tree_list::TreeState};
use maestro_theme::{LiveTheme, ThemeColor};
use maestro_tui::{TruncateOptions, interface::TuiRuntime, truncate_to_width};
use std::io;
impl TreeState {
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
        let window =
            super::super::list_window(self.selected, self.visible.len(), self.max_visible_lines);
        let (start, end) = (window.start, window.end);
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
        let node = &self.visible[index];
        let selected = index == self.selected;
        let fg = |key, text: &str| theme.fg(&key, text).map_err(io::Error::other);
        let cursor = if selected {
            fg(ThemeColor::Accent, "› ")?
        } else {
            "  ".to_owned()
        };
        let geometry = &self.structure.rows[index];
        let fold_marker = if node.entry()["id"]
            .as_str()
            .is_some_and(|id| self.folded.contains(id))
            && (!geometry.show_connector || geometry.virtual_child)
        {
            fg(ThemeColor::Accent, "⊞ ")?
        } else {
            String::new()
        };
        let active = node.entry()["id"]
            .as_str()
            .is_some_and(|id| self.active_path.contains(id));
        let marker = if active {
            fg(ThemeColor::Accent, "• ")?
        } else {
            String::new()
        };
        let label = node
            .label()
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(|label| fg(ThemeColor::Warning, &format!("[{label}] ")))
            .transpose()?
            .unwrap_or_default();
        let timestamp = self.label_time(node, theme)?;
        let content = if node.entry()["type"] == "message"
            && node.entry()["message"]["role"] == "toolResult"
        {
            fg(
                ThemeColor::Muted,
                &self.tool_result(&node.entry()["message"], runtime)?,
            )?
        } else {
            super::entry_text::entry(node.entry(), theme, styles)?
        };
        let content = maestro_theme::Theme::bold(&content, selected && styles);
        let line = cursor
            + &fg(ThemeColor::Dim, &self.prefix(index))?
            + &fold_marker
            + &marker
            + &label
            + &timestamp
            + &content;
        if selected {
            theme
                .bg(&maestro_theme::ThemeBg::SelectedBg, &line)
                .map_err(io::Error::other)
        } else {
            Ok(line)
        }
    }
    /// Formats only eligible timestamp segments, with no borrow across the clock.
    fn label_time(
        &self,
        node: &super::SessionTreeNode,
        theme: &maestro_theme::Theme,
    ) -> io::Result<String> {
        if !self.show_label_timestamps
            || node
                .label()
                .as_ref()
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
        {
            return Ok(String::new());
        }
        let timestamp = node
            .label_timestamp()
            .as_ref()
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let Some(timestamp) = timestamp.and_then(|s| maestro_app::parse_timestamp_millis(&s))
        else {
            return Ok(String::new());
        };
        let now = (self.now)();
        let Some(text) = format_label_timestamp(timestamp, now) else {
            return Ok(String::new());
        };
        theme
            .fg(&ThemeColor::Muted, &format!("{text} "))
            .map_err(io::Error::other)
    }
    /// Three-cell levels place continuing gutters before branch connectors.
    fn prefix(&self, index: usize) -> String {
        let row = &self.structure.rows[index];
        let mut prefix = maestro_app::export_html::template::build_tree_prefix(
            row,
            self.structure.multiple_roots,
        );
        let foldable = self.visible[index].entry()["id"]
            .as_str()
            .is_some_and(|id| self.structure.is_foldable(id));
        let folded = self.visible[index].entry()["id"]
            .as_str()
            .is_some_and(|id| self.folded.contains(id));
        if (foldable || folded)
            && let Some(position) = prefix.find('─')
        {
            prefix.replace_range(
                position..position + '─'.len_utf8(),
                if folded { "⊞" } else { "⊟" },
            );
        }
        prefix
    }
    /// The badge for the nondefault category selection.
    fn status(&self) -> String {
        let filter = match self.mode {
            FilterMode::Default => "",
            FilterMode::NoTools => " [no-tools]",
            FilterMode::UserOnly => " [user]",
            FilterMode::LabeledOnly => " [labeled]",
            FilterMode::All => " [all]",
        };
        format!(
            "{filter}{}",
            if self.show_label_timestamps {
                " [+label time]"
            } else {
                ""
            }
        )
    }
}

/// Projects a valid instant onto local calendar branches.
fn format_label_timestamp(millis: i64, now: std::time::SystemTime) -> Option<String> {
    use chrono::{Datelike, Local};
    let date = chrono::DateTime::from_timestamp_millis(millis)?.with_timezone(&Local);
    let now = chrono::DateTime::<Local>::from(now);
    let time = date.format("%H:%M");
    Some(if date.date_naive() == now.date_naive() {
        time.to_string()
    } else if date.year() == now.year() {
        format!("{}/{} {time}", date.month(), date.day())
    } else {
        let year = date.year().to_string();
        format!(
            "{}/{}/{} {time}",
            &year[year.len().saturating_sub(2)..],
            date.month(),
            date.day()
        )
    })
}
