//! Renderer image projection with terminal-local payload fingerprints.

use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
};

use super::{
    command::{Compression, Format},
    image::Image,
    render::PlacementRenderInfo,
    storage::{Location, PlacementId},
};
use crate::{Screen, Terminal, point::Tag};

/// Pixel encoding exposed to an image renderer.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum KittyImageFormat {
    /// Three eight-bit colour channels.
    Rgb,
    /// Three colour channels with eight-bit alpha.
    Rgba,
    /// Encoded PNG bytes.
    Png,
}

/// Image identity and payload metadata supplied to the caller's data filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KittyImageDescriptor {
    /// Protocol image identity.
    pub image_id: u32,
    /// Physical or synthesized placeholder identity.
    pub placement_id: u32,
    /// Image width in pixels.
    pub image_width: u32,
    /// Image height in pixels.
    pub image_height: u32,
    /// Payload encoding.
    pub format: KittyImageFormat,
    /// Full payload length, even when data is omitted.
    pub data_len: usize,
    /// Fingerprint of dimensions, format and the entire payload.
    pub data_fingerprint: u64,
}

/// Visible image placement ready for transport to a renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KittyImagePlacement {
    /// Protocol image identity.
    pub image_id: u32,
    /// Physical or synthesized placeholder identity.
    pub placement_id: u32,
    /// Stacking order, from lowest to highest.
    pub z: i32,
    /// Horizontal in-cell destination offset.
    pub x_offset: u32,
    /// Vertical in-cell destination offset.
    pub y_offset: u32,
    /// Image width in pixels.
    pub image_width: u32,
    /// Image height in pixels.
    pub image_height: u32,
    /// Payload encoding.
    pub format: KittyImageFormat,
    /// Full payload length, even when data is omitted.
    pub data_len: usize,
    /// Fingerprint of dimensions, format and the entire payload.
    pub data_fingerprint: u64,
    /// Owned bytes, empty when the caller already has the payload.
    pub data: Vec<u8>,
    /// Destination and source geometry relative to the viewport.
    pub render: PlacementRenderInfo,
}

/// Last fingerprint for one independently stamped image payload.
#[derive(Clone, Copy)]
struct FingerprintEntry {
    /// Image generation, unaffected by placement or viewport changes.
    generation: u64,
    /// Hash of the full image record.
    fingerprint: u64,
}

/// Projection state owned by one terminal, shared across its screen switches.
#[derive(Default)]
pub(crate) struct Projection {
    /// Storage generation observed with no placements, not merely none visible.
    empty_generation: Option<u64>,
    /// Fingerprints retained only for images in the last visible projection.
    fingerprints: HashMap<u32, FingerprintEntry>,
}

impl Projection {
    /// Reuse an image fingerprint until its payload generation changes.
    fn descriptor(&mut self, image: &Image, placement_id: u32) -> Option<KittyImageDescriptor> {
        if image.compression != Compression::None {
            return None;
        }
        let format = match image.format {
            Format::Rgb => KittyImageFormat::Rgb,
            Format::Rgba => KittyImageFormat::Rgba,
            Format::Png => KittyImageFormat::Png,
            Format::Gray | Format::GrayAlpha => return None,
        };
        let entry = self
            .fingerprints
            .entry(image.id)
            .or_insert(FingerprintEntry {
                generation: 0,
                fingerprint: 0,
            });
        if entry.generation != image.generation {
            let mut hasher = DefaultHasher::new();
            image.data.len().hash(&mut hasher);
            image.width.hash(&mut hasher);
            image.height.hash(&mut hasher);
            format.hash(&mut hasher);
            image.data.hash(&mut hasher);
            *entry = FingerprintEntry {
                generation: image.generation,
                fingerprint: hasher.finish(),
            };
        }
        Some(KittyImageDescriptor {
            image_id: image.id,
            placement_id,
            image_width: image.width,
            image_height: image.height,
            format,
            data_len: image.data.len(),
            data_fingerprint: entry.fingerprint,
        })
    }
}

impl Terminal {
    /// Project visible physical and placeholder placements in ascending z order.
    ///
    /// The filter sees a full-payload fingerprint cached per image generation.
    /// Returning false omits only the copied bytes. Cache entries are pruned to
    /// visible images. Only a generation observed without any stored placements
    /// is skipped; hidden placements can become visible without a new generation.
    pub fn kitty_image_placements_with_data_filter(
        &mut self,
        mut needs_data: impl FnMut(KittyImageDescriptor) -> bool,
    ) -> Vec<KittyImagePlacement> {
        let cell = self.image_cell_size();
        let screen = self.screens.active();
        let storage = &screen.kitty_images;
        let cache = &mut self.kitty_projection;
        if storage.generation == 0 || cache.empty_generation == Some(storage.generation) {
            return Vec::new();
        }
        if storage.placements.is_empty() {
            cache.empty_generation = Some(storage.generation);
            cache.fingerprints.clear();
            return Vec::new();
        }
        let mut result = Vec::new();
        for (key, placement) in &storage.placements {
            if placement.location == Location::Virtual {
                continue;
            }
            let Some(image) = storage.image_by_id(key.image_id) else {
                continue;
            };
            let render = placement.render_info(image, &screen.pages, cell);
            if !render.viewport_visible {
                continue;
            }
            let placement_id = match key.placement_id {
                PlacementId::External(id) | PlacementId::Internal(id) => id,
            };
            let Some(descriptor) = cache.descriptor(image, placement_id) else {
                continue;
            };
            let data = if needs_data(descriptor) {
                image.data.clone()
            } else {
                Vec::new()
            };
            result.push(KittyImagePlacement::new(
                descriptor, data, render, placement,
            ));
        }
        result.extend(cache.placeholders(screen, cell, &mut needs_data));
        result.sort_by_key(|placement| placement.z);
        let live: HashSet<_> = result.iter().map(|placement| placement.image_id).collect();
        cache.fingerprints.retain(|id, _| live.contains(id));
        result
    }
}

impl KittyImagePlacement {
    /// Assemble a transport record without duplicating payload metadata extraction.
    fn new(
        descriptor: KittyImageDescriptor,
        data: Vec<u8>,
        render: PlacementRenderInfo,
        placement: &super::storage::Placement,
    ) -> Self {
        Self {
            image_id: descriptor.image_id,
            placement_id: descriptor.placement_id,
            image_width: descriptor.image_width,
            image_height: descriptor.image_height,
            format: descriptor.format,
            data_len: descriptor.data_len,
            data_fingerprint: descriptor.data_fingerprint,
            z: placement.z,
            x_offset: placement.x_offset,
            y_offset: placement.y_offset,
            data,
            render,
        }
    }
}

impl Projection {
    /// Reuse retained placeholder decoding and fragment geometry for visible rows.
    fn placeholders(
        &mut self,
        screen: &Screen,
        cell: (u32, u32),
        needs_data: &mut impl FnMut(KittyImageDescriptor) -> bool,
    ) -> Vec<KittyImagePlacement> {
        let pages = &screen.pages;
        let storage = &screen.kitty_images;
        let start = pages.get_top_left(Tag::Viewport);
        let limit = start.down(pages, usize::from(pages.rows() - 1));
        let mut iterator = super::unicode::placement_iterator(start, limit);
        let mut result = Vec::new();
        while let Some(run) = iterator.next(pages) {
            let Some(image) = storage.image_by_id(run.image_id) else {
                continue;
            };
            let Some(prototype) = run.prototype(storage) else {
                continue;
            };
            let Ok(fragment) = run.render_placement(storage, image, cell) else {
                continue;
            };
            if fragment.dest_width == 0 || fragment.dest_height == 0 {
                continue;
            }
            let Some(point) = pages.point_from_pin(Tag::Viewport, run.pin) else {
                continue;
            };
            let coord = point.coord();
            let render = placeholder_render_info(fragment, run, coord);
            let id = synthetic_placement_id(run, coord);
            let Some(descriptor) = self.descriptor(image, id) else {
                continue;
            };
            let data = if needs_data(descriptor) {
                image.data.clone()
            } else {
                Vec::new()
            };
            let mut placement = KittyImagePlacement::new(descriptor, data, render, prototype);
            placement.x_offset = fragment.offset_x;
            placement.y_offset = fragment.offset_y;
            result.push(placement);
        }
        result
    }
}

/// Stable placeholder identity incorporating its image fragment and viewport origin.
fn synthetic_placement_id(run: super::unicode::Placement, coord: crate::point::Coordinate) -> u32 {
    let mut hasher = DefaultHasher::new();
    run.image_id.hash(&mut hasher);
    run.placement_id.hash(&mut hasher);
    run.row.hash(&mut hasher);
    run.col.hash(&mut hasher);
    run.width.hash(&mut hasher);
    coord.x.hash(&mut hasher);
    // Viewport rows are bounded by the terminal's u16 height.
    u16::try_from(coord.y).unwrap_or(u16::MAX).hash(&mut hasher);
    u32::try_from(hasher.finish() & u64::from(u32::MAX)).unwrap_or(0) % 900_000 + 1
}

/// Convert the shared fragment geometry without re-decoding placeholder cells.
fn placeholder_render_info(
    fragment: super::render::RenderPlacement,
    run: super::unicode::Placement,
    coord: crate::point::Coordinate,
) -> PlacementRenderInfo {
    PlacementRenderInfo {
        pixel_width: fragment.dest_width,
        pixel_height: fragment.dest_height,
        grid_cols: run.width,
        grid_rows: run.height,
        viewport_col: i64::from(coord.x),
        viewport_row: i64::from(coord.y),
        viewport_visible: true,
        source_x: fragment.source_x,
        source_y: fragment.source_y,
        source_width: fragment.source_width,
        source_height: fragment.source_height,
    }
}

#[cfg(test)]
mod tests;
