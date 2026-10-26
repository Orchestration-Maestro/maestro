//! Host-owned overlay placement shared by every retained alias.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::ExtensionResult;
use serde::{Deserialize, Serialize};

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

/// A size in cells or percentage text, kept as supplied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeValue {
    /// Absolute cells.
    Cells(i64),
    /// Percentage resolved by the host against the viewport.
    Percentage(String),
}

/// Margin at each viewport edge; absent and negative sides resolve to zero on the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayMargin {
    /// Top margin.
    pub top: Option<i64>,
    /// Right margin.
    pub right: Option<i64>,
    /// Bottom margin.
    pub bottom: Option<i64>,
    /// Left margin.
    pub left: Option<i64>,
}

/// A uniform margin or one margin per edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverlayMarginValue {
    /// One value for every edge.
    Uniform(i64),
    /// Individual edge margins.
    Sides(OverlayMargin),
}

/// Overlay placement; absent members keep the toolkit defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayOptions {
    /// Width in cells or a viewport percentage.
    pub width: Option<SizeValue>,
    /// Minimum width in cells.
    pub min_width: Option<i64>,
    /// Maximum height in rows or a viewport percentage.
    pub max_height: Option<SizeValue>,
    /// Anchor, centered when absent.
    pub anchor: Option<OverlayAnchor>,
    /// Horizontal offset from the anchor.
    pub offset_x: Option<i64>,
    /// Vertical offset from the anchor.
    pub offset_y: Option<i64>,
    /// Absolute or percentage row position.
    pub row: Option<SizeValue>,
    /// Absolute or percentage column position.
    pub col: Option<SizeValue>,
    /// Margin from the terminal edges.
    pub margin: Option<OverlayMarginValue>,
    /// Whether showing the overlay leaves keyboard focus unchanged.
    pub non_capturing: Option<bool>,
}

/// Reads and writes the host's shared placement record.
pub(crate) trait OverlayOptionsPort {
    /// Reads current placement.
    fn get(&self) -> ExtensionResult<OverlayOptions>;
    /// Replaces placement without changing visibility policy.
    fn set(&self, options: OverlayOptions) -> ExtensionResult<()>;
}

/// Retained host options; clones share the original native record.
#[derive(Clone)]
pub struct OverlayOptionsHandle(pub(crate) super::widgets::Supplied<dyn OverlayOptionsPort>);
impl OverlayOptionsHandle {
    /// Creates a shared placement record on the host.
    /// # Errors
    /// Returns the host's rejection.
    pub fn new(options: OverlayOptions) -> ExtensionResult<Self> {
        super::widgets::host()?.overlay_options(options)
    }
    /// Reads the host's current placement.
    /// # Errors
    /// Returns the host's rejection.
    pub fn get(&self) -> ExtensionResult<OverlayOptions> {
        self.0.function.get()
    }
    /// Replaces the shared placement, including after an overlay is shown.
    /// # Errors
    /// Returns the host's rejection.
    pub fn set(&self, options: OverlayOptions) -> ExtensionResult<()> {
        self.0.function.set(options)
    }
}
