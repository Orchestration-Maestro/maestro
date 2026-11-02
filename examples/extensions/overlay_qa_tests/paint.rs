//! Themed borders, fitted rows and key checks shared by every demonstration view.
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, Theme, keys::matches_key, truncate_to_width,
    utils::TruncateOptions, visible_width,
};

/// One row inside a border: fitted text, or a divider across the inner width.
pub enum Row {
    /// Text truncated with an ellipsis and padded to the inner width.
    Text(String),
    /// A horizontal divider joined to both sides of the border.
    Divider,
}

/// Host text operations and the view's theme.
#[derive(Clone)]
pub struct Paint {
    /// Host helpers for widths and key names.
    pub api: ExtensionAPI,
    /// The theme the view's factory received.
    pub theme: Theme,
}

/// Cells between the two side borders of a view `width` cells wide.
pub fn inner(width: u32) -> usize {
    usize::try_from(width.saturating_sub(2).max(1)).unwrap_or(1)
}

impl Paint {
    /// Colours text with a theme foreground.
    pub fn fg(&self, color: &str, text: &str) -> ExtensionResult<String> {
        self.theme.fg(color, text)
    }
    /// Truncates with an ellipsis and pads to exactly `width` cells.
    pub fn fit(&self, text: &str, width: usize) -> ExtensionResult<String> {
        let pad = TruncateOptions {
            ellipsis: "...",
            pad: true,
        };
        truncate_to_width(&self.api, text, width, pad)
    }
    /// Whether the input is one of the named keys.
    pub fn is(&self, data: &str, keys: &[&str]) -> ExtensionResult<bool> {
        for key in keys {
            if matches_key(&self.api, data, key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    /// Whether the input closes a view: Escape or Ctrl+C.
    pub fn closes(&self, data: &str) -> ExtensionResult<bool> {
        self.is(data, &["escape", "ctrl+c"])
    }
    /// Draws rows inside a border of one colour, without a title.
    pub fn framed(&self, color: &str, rows: Vec<Row>, width: u32) -> ExtensionResult<Vec<String>> {
        let inner = inner(width);
        let rule = "─".repeat(inner);
        let mut lines = vec![self.fg(color, &format!("╭{rule}╮"))?];
        for row in rows {
            lines.push(match row {
                Row::Text(text) => {
                    let side = self.fg(color, "│")?;
                    format!("{side}{}{side}", self.fit(&text, inner)?)
                }
                Row::Divider => format!(
                    "{}{}{}",
                    self.fg(color, "├")?,
                    self.fg(color, &rule)?,
                    self.fg(color, "┤")?
                ),
            });
        }
        lines.push(self.fg(color, &format!("╰{rule}╯"))?);
        Ok(lines)
    }
    /// Draws rows inside a border with an accented title centred in the top edge.
    pub fn boxed(
        &self,
        rows: Vec<String>,
        width: u32,
        title: &str,
    ) -> ExtensionResult<Vec<String>> {
        let inner = inner(width);
        let title = truncate_to_width(
            &self.api,
            &format!(" {title} "),
            inner,
            TruncateOptions::default(),
        )?;
        let left = inner.saturating_sub(visible_width(&self.api, &title)?) / 2;
        let right = inner.saturating_sub(visible_width(&self.api, &title)? + left);
        let mut lines = vec![format!(
            "{}{}{}",
            self.fg("border", &format!("╭{}", "─".repeat(left)))?,
            self.fg("accent", &title)?,
            self.fg("border", &format!("{}╮", "─".repeat(right)))?
        )];
        let side = self.fg("border", "│")?;
        for row in rows {
            lines.push(format!("{side}{}{side}", self.fit(&row, inner)?));
        }
        lines.push(self.fg("border", &format!("╰{}╯", "─".repeat(inner)))?);
        Ok(lines)
    }
    /// Text in the dim foreground.
    pub fn dim(&self, text: &str) -> ExtensionResult<String> {
        self.fg("dim", text)
    }
    /// One leading space followed by dim text, the common hint row.
    pub fn hint(&self, text: &str) -> ExtensionResult<String> {
        Ok(format!(" {}", self.dim(text)?))
    }
}
