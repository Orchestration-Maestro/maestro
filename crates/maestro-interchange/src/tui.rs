//! Toolkit records a guest shares with its host: completion suggestions, list items, editor and
//! overlay options, key phases, match scores and text measured in terminal columns.
//!
//! The toolkit and guest authoring each re-export these records at their own paths. A record
//! with a type parameter has one member whose type differs between the two sides; each side
//! names its own form through one alias.
use serde::{Deserialize, Serialize};

/// Candidates together with the text they replace.
///
/// `I` is the candidate record of the side that builds the suggestions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutocompleteSuggestions<I> {
    /// Candidates; an empty list differs from having no suggestions at all.
    pub items: Vec<I>,
    /// The text being matched, such as `/` or `src/`.
    pub prefix: String,
}

/// Text selected by terminal columns and the cells it actually occupies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnSlice {
    /// The selected graphemes with their styling. A non-empty selection starts with the
    /// escapes written before its first grapheme and closes a hyperlink still open at its end.
    pub text: String,
    /// Terminal cells the selected graphemes occupy.
    pub width: usize,
}

/// Editor options; an absent member keeps the editor's default.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EditorOptions {
    /// Columns of padding on each side. A finite value is floored and clamped at zero; a
    /// nonfinite value selects zero.
    pub padding_x: Option<f64>,
    /// Rows the completion list shows. A finite value is floored and clamped to 3..20; a
    /// nonfinite value selects 5.
    pub autocomplete_max_visible: Option<f64>,
}

/// The text on both sides of an excluded range of terminal columns, such as an overlay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSegments {
    /// The graphemes that start before the excluded range.
    pub before: String,
    /// Cells of [`before`](Self::before).
    pub before_width: usize,
    /// The graphemes that start in the requested range after the excluded one.
    pub after: String,
    /// Cells of [`after`](Self::after).
    pub after_width: usize,
}

/// Whether a query matches, together with its ranking score.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FuzzyMatch {
    /// Whether the query or its letter/digit-swapped form matched.
    pub matches: bool,
    /// Lower matching scores rank first; consecutive and boundary matches score lower.
    pub score: f64,
}

/// The phase of an enhanced key report: the key going down, repeating or going up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEventType {
    /// The key went down.
    Press,
    /// The key is held down and repeating.
    Repeat,
    /// The key went up.
    Release,
}

/// Point of the viewport an overlay is anchored to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayAnchor {
    /// Centered in both directions.
    Center,
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
    /// Centered along the top edge.
    TopCenter,
    /// Centered along the bottom edge.
    BottomCenter,
    /// Centered along the left edge.
    LeftCenter,
    /// Centered along the right edge.
    RightCenter,
}

/// One choice of a selection list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectItem {
    /// The identity the list filters by and reports.
    pub value: String,
    /// Primary display text; an empty label displays the value.
    pub label: String,
    /// The shown description, when there is one.
    pub description: Option<String>,
}

/// One setting of a settings list, with its optional activation behavior.
///
/// `S` is the submenu factory of the side that builds the list.
#[derive(Clone)]
pub struct SettingItem<S> {
    /// The setting's identity; duplicate identities are permitted.
    pub id: String,
    /// The shown label.
    pub label: String,
    /// The description shown while the row is selected.
    pub description: Option<String>,
    /// The shown current value.
    pub current_value: String,
    /// Values that activation cycles through.
    pub values: Option<Vec<String>>,
    /// A submenu that activation opens, taking precedence over cycling.
    pub submenu: Option<S>,
}

/// Settings list options.
#[derive(Clone, Copy, Default)]
pub struct SettingsListOptions {
    /// Shows a search row that filters by label with the shared matcher.
    pub enable_search: bool,
}

/// A text decoration of Markdown's base style.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDecoration {
    /// Bold text.
    Bold,
    /// Italic text.
    Italic,
    /// Struck-through text.
    Strikethrough,
    /// Underlined text.
    Underline,
}

/// How truncation to a terminal width marks and fills a shortened text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TruncateOptions<'a> {
    /// Text appended after the kept prefix when something was cut, measured in terminal cells.
    pub ellipsis: &'a str,
    /// Whether the result is padded with spaces up to the full width, measured on the
    /// finished result.
    pub pad: bool,
}

impl Default for TruncateOptions<'_> {
    fn default() -> Self {
        Self {
            ellipsis: "...",
            pad: false,
        }
    }
}
