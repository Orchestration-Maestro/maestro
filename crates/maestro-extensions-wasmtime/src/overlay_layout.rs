//! Overlay placement records cross the boundary without moving toolkit layout into the adapter.
use maestro_extensions::widgets::{
    OverlayAnchor, OverlayMargin, OverlayMarginValue, OverlayOptions, SizeValue,
};
use serde::{Deserialize, Serialize};

/// Declares the wire anchors and their direct native counterparts.
macro_rules! anchors {
    ($($anchor:ident),* $(,)?) => {
        /// Anchor names on the canonical boundary.
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        enum Anchor { $(#[doc = "The corresponding viewport anchor."] $anchor,)* }
        impl From<Anchor> for OverlayAnchor {
            fn from(anchor: Anchor) -> Self { match anchor { $(Anchor::$anchor => Self::$anchor,)* } }
        }
        impl From<OverlayAnchor> for Anchor {
            fn from(anchor: OverlayAnchor) -> Self { match anchor { $(OverlayAnchor::$anchor => Self::$anchor,)* } }
        }
    };
}
anchors! { Center, TopLeft, TopRight, BottomLeft, BottomRight, TopCenter, BottomCenter, LeftCenter, RightCenter }

/// Cells or percentage text on the boundary.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Size {
    /// Absolute cells.
    Cells(i64),
    /// Percentage text.
    Percentage(String),
}
impl Size {
    /// Converts cells without narrowing silently.
    fn native(self) -> Result<SizeValue, String> {
        Ok(match self {
            Self::Cells(value) => SizeValue::Cells(cells(value)?),
            Self::Percentage(value) => SizeValue::Percentage(value),
        })
    }
    /// Keeps native cells and percentage text unchanged.
    fn from_native(value: SizeValue) -> Result<Self, String> {
        Ok(match value {
            SizeValue::Cells(value) => Self::Cells(wide(value)?),
            SizeValue::Percentage(value) => Self::Percentage(value),
        })
    }
}

/// One margin or the four edge margins on the boundary.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Margin {
    /// The same margin on every edge.
    Uniform(i64),
    /// Top, right, bottom and left margins.
    Sides(Sides),
}

/// Margins at each viewport edge.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sides {
    /// Top margin.
    top: Option<i64>,
    /// Right margin.
    right: Option<i64>,
    /// Bottom margin.
    bottom: Option<i64>,
    /// Left margin.
    left: Option<i64>,
}
impl Margin {
    /// Converts the supplied margins to native cells.
    fn native(self) -> Result<OverlayMarginValue, String> {
        Ok(match self {
            Self::Uniform(value) => OverlayMarginValue::Uniform(cells(value)?),
            Self::Sides(sides) => OverlayMarginValue::Sides(OverlayMargin {
                top: sides.top.map(cells).transpose()?,
                right: sides.right.map(cells).transpose()?,
                bottom: sides.bottom.map(cells).transpose()?,
                left: sides.left.map(cells).transpose()?,
            }),
        })
    }
    /// Keeps the native margins unchanged.
    fn from_native(value: OverlayMarginValue) -> Result<Self, String> {
        Ok(match value {
            OverlayMarginValue::Uniform(value) => Self::Uniform(wide(value)?),
            OverlayMarginValue::Sides(sides) => Self::Sides(Sides {
                top: sides.top.map(wide).transpose()?,
                right: sides.right.map(wide).transpose()?,
                bottom: sides.bottom.map(wide).transpose()?,
                left: sides.left.map(wide).transpose()?,
            }),
        })
    }
}

/// The placement record; visibility is owned separately as an executable callback.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    /// Width.
    width: Option<Size>,
    /// Minimum width.
    min_width: Option<i64>,
    /// Maximum height.
    max_height: Option<Size>,
    /// Viewport anchor.
    anchor: Option<Anchor>,
    /// Horizontal offset.
    offset_x: Option<i64>,
    /// Vertical offset.
    offset_y: Option<i64>,
    /// Row position.
    row: Option<Size>,
    /// Column position.
    col: Option<Size>,
    /// Edge margins.
    margin: Option<Margin>,
    /// Whether showing leaves focus unchanged.
    non_capturing: Option<bool>,
}
impl Layout {
    /// Converts placement, leaving all layout policy with the toolkit.
    fn native(self) -> Result<OverlayOptions, String> {
        Ok(OverlayOptions {
            width: self.width.map(Size::native).transpose()?,
            min_width: self.min_width.map(cells).transpose()?,
            max_height: self.max_height.map(Size::native).transpose()?,
            anchor: self.anchor.map(Into::into),
            offset_x: self.offset_x.map(cells).transpose()?,
            offset_y: self.offset_y.map(cells).transpose()?,
            row: self.row.map(Size::native).transpose()?,
            col: self.col.map(Size::native).transpose()?,
            margin: self.margin.map(Margin::native).transpose()?,
            non_capturing: self.non_capturing,
            visible: None,
        })
    }
    /// Reads placement without invoking its visibility callback.
    fn from_native(value: &OverlayOptions) -> Result<Self, String> {
        Ok(Self {
            width: value.width.clone().map(Size::from_native).transpose()?,
            min_width: value.min_width.map(wide).transpose()?,
            max_height: value
                .max_height
                .clone()
                .map(Size::from_native)
                .transpose()?,
            anchor: value.anchor.map(Into::into),
            offset_x: value.offset_x.map(wide).transpose()?,
            offset_y: value.offset_y.map(wide).transpose()?,
            row: value.row.clone().map(Size::from_native).transpose()?,
            col: value.col.clone().map(Size::from_native).transpose()?,
            margin: value.margin.map(Margin::from_native).transpose()?,
            non_capturing: value.non_capturing,
        })
    }
}

/// Parses the plain placement record at the component boundary.
pub(crate) fn decode(encoded: &str) -> Result<OverlayOptions, String> {
    serde_json::from_str::<Layout>(encoded)
        .map_err(|error| error.to_string())?
        .native()
}
/// Encodes the current native placement.
pub(crate) fn encode(options: &OverlayOptions) -> Result<String, String> {
    serde_json::to_string(&Layout::from_native(options)?).map_err(|error| error.to_string())
}
/// Converts supplied cells to the host's cell count.
fn cells(value: i64) -> Result<isize, String> {
    isize::try_from(value).map_err(|error| error.to_string())
}
/// Converts native cells to the boundary's fixed-width count.
fn wide(value: isize) -> Result<i64, String> {
    i64::try_from(value).map_err(|error| error.to_string())
}
