//! Immutable foreground/background theme prefixes and authored metadata.
use indexmap::IndexMap;
use maestro_request::source_info::SourceInfo;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
#[cfg(not(target_arch = "wasm32"))]
mod collation;
mod colors;
mod loading;
mod registry;
#[cfg(not(target_arch = "wasm32"))]
pub use loading::NativeThemeOperations;
pub use loading::{ThemeOperations, load_theme_from_path};
pub use registry::{ThemeDirectories, ThemeInfo, ThemeState};

/// Terminal color conversion mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Emit RGB colors directly.
    Truecolor,
    /// Quantize RGB colors to the terminal palette.
    Color256,
}
/// Authored string color or explicit palette index.
#[derive(Debug, Clone)]
pub enum ColorValue {
    /// Empty/default color or hexadecimal RGB string.
    String(String),
    /// Explicit terminal palette index.
    Index(u8),
}
/// Typed foreground key, with literal names for runtime extensions.
#[derive(Debug, Clone)]
pub enum ThemeColor {
    /// The `accent` token.
    Accent,
    /// The `border` token.
    Border,
    /// The `borderAccent` token.
    BorderAccent,
    /// The `borderMuted` token.
    BorderMuted,
    /// The `success` token.
    Success,
    /// The `error` token.
    Error,
    /// The `warning` token.
    Warning,
    /// The `muted` token.
    Muted,
    /// The `dim` token.
    Dim,
    /// The `text` token.
    Text,
    /// The `thinkingText` token.
    ThinkingText,
    /// The `userMessageText` token.
    UserMessageText,
    /// The `customMessageText` token.
    CustomMessageText,
    /// The `customMessageLabel` token.
    CustomMessageLabel,
    /// The `toolTitle` token.
    ToolTitle,
    /// The `toolOutput` token.
    ToolOutput,
    /// The `mdHeading` token.
    MdHeading,
    /// The `mdLink` token.
    MdLink,
    /// The `mdLinkUrl` token.
    MdLinkUrl,
    /// The `mdCode` token.
    MdCode,
    /// The `mdCodeBlock` token.
    MdCodeBlock,
    /// The `mdCodeBlockBorder` token.
    MdCodeBlockBorder,
    /// The `mdQuote` token.
    MdQuote,
    /// The `mdQuoteBorder` token.
    MdQuoteBorder,
    /// The `mdHr` token.
    MdHr,
    /// The `mdListBullet` token.
    MdListBullet,
    /// The `toolDiffAdded` token.
    ToolDiffAdded,
    /// The `toolDiffRemoved` token.
    ToolDiffRemoved,
    /// The `toolDiffContext` token.
    ToolDiffContext,
    /// The `syntaxComment` token.
    SyntaxComment,
    /// The `syntaxKeyword` token.
    SyntaxKeyword,
    /// The `syntaxFunction` token.
    SyntaxFunction,
    /// The `syntaxVariable` token.
    SyntaxVariable,
    /// The `syntaxString` token.
    SyntaxString,
    /// The `syntaxNumber` token.
    SyntaxNumber,
    /// The `syntaxType` token.
    SyntaxType,
    /// The `syntaxOperator` token.
    SyntaxOperator,
    /// The `syntaxPunctuation` token.
    SyntaxPunctuation,
    /// The `thinkingOff` token.
    ThinkingOff,
    /// The `thinkingMinimal` token.
    ThinkingMinimal,
    /// The `thinkingLow` token.
    ThinkingLow,
    /// The `thinkingMedium` token.
    ThinkingMedium,
    /// The `thinkingHigh` token.
    ThinkingHigh,
    /// The `thinkingXhigh` token.
    ThinkingXhigh,
    /// The `bashMode` token.
    BashMode,
    /// An authored literal key.
    Named(String),
}
impl ThemeColor {
    /// Consume the token into its literal map key.
    fn into_key(self) -> String {
        match self {
            Self::Named(name) => name,
            other => other.as_str().to_owned(),
        }
    }
    /// Return the literal map key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Accent => "accent",
            Self::Border => "border",
            Self::BorderAccent => "borderAccent",
            Self::BorderMuted => "borderMuted",
            Self::Success => "success",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Muted => "muted",
            Self::Dim => "dim",
            Self::Text => "text",
            Self::ThinkingText => "thinkingText",
            Self::UserMessageText => "userMessageText",
            Self::CustomMessageText => "customMessageText",
            Self::CustomMessageLabel => "customMessageLabel",
            Self::ToolTitle => "toolTitle",
            Self::ToolOutput => "toolOutput",
            Self::MdHeading => "mdHeading",
            Self::MdLink => "mdLink",
            Self::MdLinkUrl => "mdLinkUrl",
            Self::MdCode => "mdCode",
            Self::MdCodeBlock => "mdCodeBlock",
            Self::MdCodeBlockBorder => "mdCodeBlockBorder",
            Self::MdQuote => "mdQuote",
            Self::MdQuoteBorder => "mdQuoteBorder",
            Self::MdHr => "mdHr",
            Self::MdListBullet => "mdListBullet",
            Self::ToolDiffAdded => "toolDiffAdded",
            Self::ToolDiffRemoved => "toolDiffRemoved",
            Self::ToolDiffContext => "toolDiffContext",
            Self::SyntaxComment => "syntaxComment",
            Self::SyntaxKeyword => "syntaxKeyword",
            Self::SyntaxFunction => "syntaxFunction",
            Self::SyntaxVariable => "syntaxVariable",
            Self::SyntaxString => "syntaxString",
            Self::SyntaxNumber => "syntaxNumber",
            Self::SyntaxType => "syntaxType",
            Self::SyntaxOperator => "syntaxOperator",
            Self::SyntaxPunctuation => "syntaxPunctuation",
            Self::ThinkingOff => "thinkingOff",
            Self::ThinkingMinimal => "thinkingMinimal",
            Self::ThinkingLow => "thinkingLow",
            Self::ThinkingMedium => "thinkingMedium",
            Self::ThinkingHigh => "thinkingHigh",
            Self::ThinkingXhigh => "thinkingXhigh",
            Self::BashMode => "bashMode",
            Self::Named(name) => name,
        }
    }
}
/// Typed background key, with literal names for runtime extensions.
#[derive(Debug, Clone)]
pub enum ThemeBg {
    /// The `selectedBg` token.
    SelectedBg,
    /// The `userMessageBg` token.
    UserMessageBg,
    /// The `customMessageBg` token.
    CustomMessageBg,
    /// The `toolPendingBg` token.
    ToolPendingBg,
    /// The `toolSuccessBg` token.
    ToolSuccessBg,
    /// The `toolErrorBg` token.
    ToolErrorBg,
    /// An authored literal key.
    Named(String),
}
impl ThemeBg {
    /// Consume the token into its literal map key.
    fn into_key(self) -> String {
        match self {
            Self::Named(name) => name,
            other => other.as_str().to_owned(),
        }
    }
    /// Return the literal map key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::SelectedBg => "selectedBg",
            Self::UserMessageBg => "userMessageBg",
            Self::CustomMessageBg => "customMessageBg",
            Self::ToolPendingBg => "toolPendingBg",
            Self::ToolSuccessBg => "toolSuccessBg",
            Self::ToolErrorBg => "toolErrorBg",
            Self::Named(name) => name,
        }
    }
}
/// Optional authored instance metadata.
#[derive(Debug, Default)]
pub struct ThemeOptions {
    /// Authored theme name.
    pub name: Option<String>,
    /// Authored source path.
    pub source_path: Option<String>,
    /// Shared resource provenance, replaceable later through [`Theme::set_source_info`].
    pub source_info: Option<Rc<RefCell<SourceInfo>>>,
}
/// Theme construction or lookup failure.
#[derive(Debug)]
pub struct ThemeError {
    /// Application-owned label.
    message: String,
    /// Native parser, I/O or schema-compilation cause.
    cause: Option<Box<dyn std::error::Error + Send + Sync>>,
}
impl ThemeError {
    /// Create an application error.
    fn message(message: String) -> Self {
        Self {
            message,
            cause: None,
        }
    }
    /// Keep a native failure as the cause, displayed as `message`.
    fn caused_by(message: String, cause: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            message,
            cause: Some(Box::new(cause)),
        }
    }
    /// Keep an I/O failure, displayed as itself.
    fn io(cause: std::io::Error) -> Self {
        Self::caused_by(cause.to_string(), cause)
    }
}
impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ThemeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &dyn std::error::Error)
    }
}
/// Prepared terminal color prefixes.
#[derive(Debug)]
pub struct Theme {
    /// Foreground prefixes keyed by literal name.
    fg_colors: IndexMap<String, String>,
    /// Background prefixes keyed by literal name.
    bg_colors: IndexMap<String, String>,
    /// Retained conversion mode.
    mode: ColorMode,
    /// Authored instance metadata; its shared provenance moved to `source_info`.
    options: ThemeOptions,
    /// Replaceable slot holding the shared provenance record.
    source_info: RefCell<Option<Rc<RefCell<SourceInfo>>>>,
}
impl Theme {
    /// Prepare the supplied foreground and background records.
    ///
    /// # Errors
    /// Returns a color error for invalid authored colors.
    pub fn new(
        fg_colors: impl IntoIterator<Item = (ThemeColor, ColorValue)>,
        bg_colors: impl IntoIterator<Item = (ThemeBg, ColorValue)>,
        mode: ColorMode,
        mut options: ThemeOptions,
    ) -> Result<Self, ThemeError> {
        let fg: IndexMap<_, _> = fg_colors
            .into_iter()
            .map(|(key, value)| (key.into_key(), value))
            .collect();
        let bg: IndexMap<_, _> = bg_colors
            .into_iter()
            .map(|(key, value)| (key.into_key(), value))
            .collect();
        let fg_colors = prepare(fg, mode, 38)?;
        let bg_colors = prepare(bg, mode, 48)?;
        let source_info = RefCell::new(options.source_info.take());
        Ok(Self {
            fg_colors,
            bg_colors,
            mode,
            options,
            source_info,
        })
    }
    /// Borrow the authored theme name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.options.name.as_deref()
    }
    /// Borrow the supplied source path without normalization.
    #[must_use]
    pub fn source_path(&self) -> Option<&str> {
        self.options.source_path.as_deref()
    }
    /// Return the shared provenance record, if the slot holds one.
    #[must_use]
    pub fn source_info(&self) -> Option<Rc<RefCell<SourceInfo>>> {
        self.source_info.borrow().clone()
    }
    /// Replace the slot; the previous record is left unchanged for its other holders.
    pub fn set_source_info(&self, source_info: Option<Rc<RefCell<SourceInfo>>>) {
        *self.source_info.borrow_mut() = source_info;
    }
    /// Return the conversion mode stored at construction.
    #[must_use]
    pub fn get_color_mode(&self) -> ColorMode {
        self.mode
    }
    /// Wrap unchanged text with the stored foreground prefix and a foreground reset.
    ///
    /// # Errors
    /// Returns an unknown-color error when the key is absent.
    pub fn fg(&self, color: &ThemeColor, text: &str) -> Result<String, ThemeError> {
        Ok(format!("{}{text}\x1b[39m", self.get_fg_ansi(color)?))
    }
    /// Wrap unchanged text with the stored background prefix and a background reset.
    ///
    /// # Errors
    /// Returns an unknown-background-color error when the key is absent.
    pub fn bg(&self, color: &ThemeBg, text: &str) -> Result<String, ThemeError> {
        Ok(format!("{}{text}\x1b[49m", self.get_bg_ansi(color)?))
    }
    /// Borrow a stored background prefix.
    ///
    /// # Errors
    /// Returns an unknown-background-color error when the key is absent.
    pub fn get_bg_ansi(&self, color: &ThemeBg) -> Result<&str, ThemeError> {
        self.bg_colors
            .get(color.as_str())
            .map(String::as_str)
            .ok_or_else(|| {
                ThemeError::message(format!(
                    "Unknown theme background color: {}",
                    color.as_str()
                ))
            })
    }
    /// Borrow a stored foreground prefix.
    ///
    /// # Errors
    /// Returns an unknown-color error when the key is absent.
    pub fn get_fg_ansi(&self, color: &ThemeColor) -> Result<&str, ThemeError> {
        self.fg_colors
            .get(color.as_str())
            .map(String::as_str)
            .ok_or_else(|| ThemeError::message(format!("Unknown theme color: {}", color.as_str())))
    }
}

/// Prepare one already-collected color plane in record order.
fn prepare(
    colors: IndexMap<String, ColorValue>,
    mode: ColorMode,
    plane: u8,
) -> Result<IndexMap<String, String>, ThemeError> {
    colors
        .into_iter()
        .map(|(key, value)| Ok((key, colors::ansi(&value, mode, plane)?)))
        .collect()
}
