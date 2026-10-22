//! Host image and placement identities.
//!
//! Every identity hashes only the raw pane identity and the payload or
//! placement signature, so the same pixels keep their host image across frames
//! and a changed payload maps to a fresh identity.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::{ClippedPlacement, HostPlacement, HostSourceKey, ImageSignature, PlacementSignature};
use crate::kitty::graphics::placements::{KittyImageFormat, KittyImagePlacement};

/// Lowest identity minted for an uploaded payload.
pub const HOST_IMAGE_ID_BASE: u32 = 10_000;

/// Host image identity of a placement's pixels.
#[must_use]
pub fn host_image_id(pane_id: u32, placement: &KittyImagePlacement) -> u32 {
    host_image_id_for_signature(
        pane_id,
        ImageSignature {
            image_width: placement.image_width,
            image_height: placement.image_height,
            format_code: kitty_format_code(placement.format),
            data_len: placement.data_len,
            data_fingerprint: placement.data_fingerprint,
        },
    )
}

/// Host image identity of an already computed payload signature.
#[must_use]
pub fn host_image_id_for_signature(pane_id: u32, signature: ImageSignature) -> u32 {
    let mut hasher = DefaultHasher::new();
    pane_id.hash(&mut hasher);
    signature.hash(&mut hasher);
    HOST_IMAGE_ID_BASE + (low_bits(hasher.finish()) % 900_000)
}

/// Host placement identity of a source's placement.
#[must_use]
pub fn host_placement_id(source_key: &HostSourceKey, placement: &KittyImagePlacement) -> u32 {
    let mut hasher = DefaultHasher::new();
    match source_key {
        HostSourceKey::Terminal { pane_id, .. } => pane_id.hash(&mut hasher),
        HostSourceKey::PaneLayer { pane_id, layer_id } => {
            "pane.graphics".hash(&mut hasher);
            pane_id.hash(&mut hasher);
            layer_id.hash(&mut hasher);
        }
    }
    placement.image_id.hash(&mut hasher);
    placement.placement_id.hash(&mut hasher);
    1 + (low_bits(hasher.finish()) % 900_000)
}

/// Kitty format code of a pixel encoding.
#[must_use]
pub fn kitty_format_code(format: KittyImageFormat) -> u32 {
    match format {
        KittyImageFormat::Rgb => 24,
        KittyImageFormat::Rgba => 32,
        KittyImageFormat::Png => 100,
    }
}

/// Payload signature of a placement in one pixel encoding.
pub(super) fn image_signature(placement: &HostPlacement, format_code: u32) -> ImageSignature {
    ImageSignature {
        image_width: placement.placement.image_width,
        image_height: placement.placement.image_height,
        format_code,
        data_len: placement.placement.data_len,
        data_fingerprint: placement.placement.data_fingerprint,
    }
}

/// Shown geometry of a clipped placement.
pub(super) fn placement_signature(
    clipped: ClippedPlacement,
    z: i32,
    scrollback_offset: u32,
) -> PlacementSignature {
    PlacementSignature {
        x: clipped.x,
        y: clipped.y,
        cols: clipped.cols,
        rows: clipped.rows,
        source_x: clipped.source_x,
        source_y: clipped.source_y,
        source_width: clipped.source_width,
        source_height: clipped.source_height,
        x_offset: clipped.x_offset,
        y_offset: clipped.y_offset,
        z,
        scrollback_offset,
    }
}

/// Low 32 bits of a finished hash.
fn low_bits(hash: u64) -> u32 {
    u32::try_from(hash & u64::from(u32::MAX)).unwrap_or(u32::MAX)
}
