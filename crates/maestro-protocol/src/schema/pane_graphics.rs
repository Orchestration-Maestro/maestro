//! Pane graphics protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
/// Pane graphics format record.
#[schemars(description = "")]
#[schemars(transform = crate::schema::string_enum)]
pub enum PaneGraphicsFormat {
    /// Png.
    #[schemars(description = "")]
    Png,
    /// Rgb.
    #[schemars(description = "")]
    Rgb,
    /// Rgba.
    #[schemars(description = "")]
    Rgba,
    /// Bgra.
    #[schemars(description = "")]
    Bgra,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane graphics set params record.
#[schemars(description = "")]
pub struct PaneGraphicsSetParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Layer id.
    #[schemars(description = "")]
    pub layer_id: Option<String>,
    #[serde(default)]
    /// Z index.
    #[schemars(description = "")]
    pub z_index: i32,
    #[serde(skip)]
    #[schemars(skip)]
    /// Owner.
    #[schemars(description = "")]
    pub owner: String,
    /// Format.
    #[schemars(description = "")]
    pub format: PaneGraphicsFormat,
    /// Image width.
    #[schemars(description = "")]
    pub image_width: u32,
    /// Image height.
    #[schemars(description = "")]
    pub image_height: u32,
    #[serde(skip)]
    /// Data.
    #[schemars(description = "")]
    pub data: Option<Vec<u8>>,
    #[serde(default)]
    /// Data base64.
    #[schemars(description = "")]
    pub data_base64: String,
    #[serde(default)]
    /// Placement.
    #[schemars(description = "")]
    pub placement: PaneGraphicsPlacementParams,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema, Default,
)]
/// Pane graphics placement params record.
#[schemars(description = "")]
pub struct PaneGraphicsPlacementParams {
    #[serde(default)]
    /// Viewport col.
    #[schemars(description = "")]
    pub viewport_col: i32,
    #[serde(default)]
    /// Viewport row.
    #[schemars(description = "")]
    pub viewport_row: i32,
    #[serde(default)]
    /// Grid cols.
    #[schemars(description = "")]
    pub grid_cols: u32,
    #[serde(default)]
    /// Grid rows.
    #[schemars(description = "")]
    pub grid_rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane graphics clear params record.
#[schemars(description = "")]
pub struct PaneGraphicsClearParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Layer id.
    #[schemars(description = "")]
    pub layer_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Pane graphics direct params record.
pub struct PaneGraphicsDirectParams {
    /// Pane id.
    pub pane_id: String,
    /// Layer id.
    pub layer_id: Option<String>,
    /// Z index.
    pub z_index: i32,
    /// Owner.
    pub owner: String,
    /// Image width.
    pub image_width: u32,
    /// Image height.
    pub image_height: u32,
    /// Format.
    pub format: PaneGraphicsFormat,
    /// Path.
    pub path: String,
    /// Sequence.
    pub sequence: u64,
    /// Revision.
    pub revision: u64,
    /// Placement.
    pub placement: PaneGraphicsPlacementParams,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
/// Pane graphics stream params record.
#[schemars(description = "")]
pub struct PaneGraphicsStreamParams {
    /// Pane id.
    #[schemars(description = "")]
    pub pane_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Layer id.
    #[schemars(description = "")]
    pub layer_id: Option<String>,
    #[serde(default)]
    /// Z index.
    #[schemars(description = "")]
    pub z_index: i32,
    #[serde(skip)]
    #[schemars(skip)]
    /// Owner.
    #[schemars(description = "")]
    pub owner: String,
}
