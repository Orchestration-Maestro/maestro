//! Borrowed export inputs and replaceable document effects.
use maestro_models::Tool;
use maestro_theme::{ThemeError, ThemeExportColors};
use serde_json::Value;
use std::{error::Error, fmt, io};

/// Output, theme and optional custom rendering selections.
#[derive(Default)]
pub struct ExportOptions<'a> {
    /// Authored output path; empty selects the default filename.
    pub output_path: Option<&'a str>,
    /// Supplied theme selection, including an empty name.
    pub theme_name: Option<&'a str>,
    /// Optional custom-output callback.
    pub tool_renderer: Option<&'a dyn ToolHtmlRenderer>,
}
impl<'a> From<&'a str> for ExportOptions<'a> {
    fn from(path: &'a str) -> Self {
        Self {
            output_path: Some(path),
            ..Self::default()
        }
    }
}
/// Optional metadata borrowed independently of an agent runtime.
pub struct ExportState<'a> {
    /// Prompt text; an empty prompt remains present.
    pub system_prompt: Option<&'a str>,
    /// Ordered tool descriptors.
    pub tools: Option<&'a [Tool]>,
}
/// Raw result fields passed to a custom renderer.
pub struct ToolHtmlResultInput<'a> {
    /// Stored content, preserving missing versus null.
    pub content: Option<&'a Value>,
    /// Stored details, preserving missing versus null.
    pub details: Option<&'a Value>,
    /// Truth of the stored error flag.
    pub is_error: bool,
}
/// Optional result renderings; empty strings remain present.
pub struct ToolHtmlResult {
    /// Collapsed output.
    pub collapsed: Option<String>,
    /// Expanded output.
    pub expanded: Option<String>,
}
/// Fallible custom-tool output callbacks, borrowing stored input.
pub trait ToolHtmlRenderer {
    /// Render a call; absent or empty output creates no record.
    ///
    /// # Errors
    /// Propagates the callback's failure without fallback rendering.
    fn render_call(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        args: Option<&Value>,
    ) -> Result<Option<String>, Box<dyn Error>>;
    /// Render a result; absence leaves the existing record unchanged.
    ///
    /// # Errors
    /// Propagates the callback's failure without fallback rendering.
    fn render_result(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        result: ToolHtmlResultInput<'_>,
    ) -> Result<Option<ToolHtmlResult>, Box<dyn Error>>;
}
/// One asset in the supplied offline document bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportAsset {
    /// HTML shell.
    Template,
    /// Stylesheet.
    Css,
    /// Application script.
    JavaScript,
    /// Markdown library script.
    Markdown,
    /// Highlighting library script.
    Highlight,
}
/// Imported bundle and theme observations.
pub trait ExportOperations {
    /// Application name used in default output filenames.
    fn app_name(&self) -> &str;
    /// Read one supplied asset.
    ///
    /// # Errors
    /// Returns the original read failure.
    fn read_asset(&self, asset: ExportAsset) -> io::Result<String>;
    /// Observe resolved colors.
    ///
    /// # Errors
    /// Returns the theme owner's failure.
    fn get_resolved_theme_colors(
        &self,
        theme_name: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError>;
    /// Observe optional explicit backgrounds.
    fn get_theme_export_colors(&self, theme_name: Option<&str>) -> ThemeExportColors;
}
/// Preconditions and failures reached while producing a document.
#[derive(Debug)]
pub enum ExportError {
    /// Session has no nonempty filename.
    InMemorySession,
    /// Selected session file is not published.
    UnpublishedSession,
    /// Input file does not exist.
    MissingFile(String),
    /// Native I/O or invalid reached session fields.
    Io(io::Error),
    /// Theme resolution failure.
    Theme(ThemeError),
    /// Payload encoding failure.
    Json(serde_json::Error),
    /// Direct renderer failure.
    Renderer(Box<dyn Error>),
}
impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InMemorySession => f.write_str("Cannot export in-memory session to HTML"),
            Self::UnpublishedSession => {
                f.write_str("Nothing to export yet - start a conversation first")
            }
            Self::MissingFile(path) => write!(f, "File not found: {path}"),
            Self::Io(error) => error.fmt(f),
            Self::Theme(error) => error.fmt(f),
            Self::Json(error) => error.fmt(f),
            Self::Renderer(error) => error.fmt(f),
        }
    }
}
impl Error for ExportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Theme(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Renderer(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<ThemeError> for ExportError {
    fn from(error: ThemeError) -> Self {
        Self::Theme(error)
    }
}
impl From<serde_json::Error> for ExportError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
