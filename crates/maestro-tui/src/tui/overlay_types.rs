//! Placement and control records for overlays; resolving them belongs to the renderer.

use std::rc::Rc;

/// Point of the viewport an overlay is anchored to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayAnchor {
    /// Centred in both directions.
    Center,
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
    /// Centred along the top edge.
    TopCenter,
    /// Centred along the bottom edge.
    BottomCenter,
    /// Centred along the left edge.
    LeftCenter,
    /// Centred along the right edge.
    RightCenter,
}

/// Margin per viewport edge; an absent side has no margin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverlayMargin {
    /// Cells kept free at the top.
    pub top: Option<usize>,
    /// Cells kept free at the right.
    pub right: Option<usize>,
    /// Cells kept free at the bottom.
    pub bottom: Option<usize>,
    /// Cells kept free at the left.
    pub left: Option<usize>,
}

/// A size in cells or a percentage of the viewport such as `"50%"`, kept as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeValue {
    /// Absolute cells.
    Cells(usize),
    /// Percentage text; this crate does not parse or resolve it.
    Percentage(String),
}

/// One margin for every side, or one per side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayMarginValue {
    /// The same margin on all four sides.
    Uniform(usize),
    /// Individual margins.
    Sides(OverlayMargin),
}

/// How an overlay is sized, positioned and shown; every member is optional.
#[derive(Clone, Default)]
pub struct OverlayOptions {
    /// Width in cells or as a percentage of the viewport width.
    pub width: Option<SizeValue>,
    /// Minimum width in cells.
    pub min_width: Option<usize>,
    /// Maximum height in rows or as a percentage of the viewport height.
    pub max_height: Option<SizeValue>,
    /// Anchor point; the renderer chooses the default.
    pub anchor: Option<OverlayAnchor>,
    /// Horizontal offset from the anchor; positive moves right.
    pub offset_x: Option<isize>,
    /// Vertical offset from the anchor; positive moves down.
    pub offset_y: Option<isize>,
    /// Row position in rows or as a percentage.
    pub row: Option<SizeValue>,
    /// Column position in cells or as a percentage.
    pub col: Option<SizeValue>,
    /// Margin kept from the viewport edges.
    pub margin: Option<OverlayMarginValue>,
    /// Decides from the viewport width and height whether the overlay shows.
    pub visible: Option<Rc<dyn Fn(usize, usize) -> bool>>,
    /// When true the overlay does not take keyboard focus on showing.
    pub non_capturing: Option<bool>,
}

/// Control over a shown overlay.
pub trait OverlayHandle {
    /// Removes the overlay for good.
    fn hide(&mut self);

    /// Hides or shows the overlay temporarily.
    fn set_hidden(&mut self, hidden: bool);

    /// Whether the overlay is temporarily hidden.
    fn is_hidden(&self) -> bool;

    /// Gives the overlay focus and brings it to the front.
    fn focus(&mut self);

    /// Returns focus to the previous target.
    fn unfocus(&mut self);

    /// Whether the overlay has focus.
    fn is_focused(&self) -> bool;
}
