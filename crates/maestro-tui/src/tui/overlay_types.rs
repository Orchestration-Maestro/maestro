//! Placement options and public controls for retained overlays.

use std::io;
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

/// Margin per viewport edge; absent and negative sides resolve to zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverlayMargin {
    /// Signed margin at the top.
    pub top: Option<isize>,
    /// Signed margin at the right.
    pub right: Option<isize>,
    /// Signed margin at the bottom.
    pub bottom: Option<isize>,
    /// Signed margin at the left.
    pub left: Option<isize>,
}

/// A size in cells or a percentage of the viewport such as `"50%"`, kept as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SizeValue {
    /// Absolute cells.
    Cells(isize),
    /// Percentage text resolved against the relevant viewport span.
    Percentage(String),
}

/// One margin for every side, or one per side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayMarginValue {
    /// The same margin on all four sides.
    Uniform(isize),
    /// Individual margins.
    Sides(OverlayMargin),
}

/// How an overlay is sized, positioned and shown; every member is optional.
#[derive(Clone, Default)]
pub struct OverlayOptions {
    /// Width in cells or as a percentage of the viewport width.
    pub width: Option<SizeValue>,
    /// Minimum width in cells.
    pub min_width: Option<isize>,
    /// Maximum height in rows or as a percentage of the viewport height.
    pub max_height: Option<SizeValue>,
    /// Anchor point; absent means center.
    pub anchor: Option<OverlayAnchor>,
    /// Horizontal offset from the anchor; positive moves right.
    pub offset_x: Option<isize>,
    /// Vertical offset from the anchor; positive moves down.
    pub offset_y: Option<isize>,
    /// Row position in rows or as a percentage.
    pub row: Option<SizeValue>,
    /// Column position in cells or as a percentage.
    pub col: Option<SizeValue>,
    /// Margins used to resolve available space and placement.
    pub margin: Option<OverlayMarginValue>,
    /// Decides from the viewport width and height whether the overlay shows.
    pub visible: Option<Rc<dyn Fn(usize, usize) -> bool>>,
    /// When true the overlay does not take keyboard focus on showing.
    pub non_capturing: Option<bool>,
}

/// Control over a shown overlay.
pub trait OverlayHandle {
    /// Removes the overlay for good.
    ///
    /// # Errors
    /// Returns cursor-hiding errors after removal and focus restoration.
    fn hide(&mut self) -> io::Result<()>;

    /// Hides or shows an attached overlay temporarily; equal flags and detached
    /// entries do nothing. Capturing entries regain focus on showing when visible.
    fn set_hidden(&mut self, hidden: bool);

    /// Whether the overlay is temporarily hidden.
    fn is_hidden(&self) -> bool;

    /// Focuses a visible attached overlay and raises its visual order, even when
    /// already focused; hidden, callback-invisible and detached entries do nothing.
    fn focus(&mut self);

    /// Yields focus when attached and focused: to the last-created visible capturing
    /// entry unless it is this entry, otherwise to an available captured predecessor.
    fn unfocus(&mut self);

    /// Whether the component identity currently owns focus, even after detachment.
    fn is_focused(&self) -> bool;
}
