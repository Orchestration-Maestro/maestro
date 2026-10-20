//! Screen-local image ownership with process-wide content generation stamps.

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};

use super::image::{Image, Limits, LoadingImage};
use crate::{PageList, page_list::TrackedPin};

/// Process-wide content stamps; zero denotes storage that has never changed.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// Obtain a unique, increasing stamp across screens, resets and terminals.
pub fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::Relaxed) + 1
}

/// A protocol placement ID or an automatically assigned, private placement ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlacementId {
    /// ID supplied by the client, always nonzero.
    External(u32),
    /// Private ID used when the client requested an anonymous placement.
    Internal(u32),
}

/// Unique identity of a placement within screen-local image storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlacementKey {
    /// Image being displayed.
    pub image_id: u32,
    /// Client or internal placement identity.
    pub placement_id: PlacementId,
}

/// Physical retained origin or an invisible placeholder prototype.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Location {
    /// Position tracked by the owning screen's page list.
    Pin(TrackedPin),
    /// Prototype used by Unicode placeholder cells.
    #[default]
    Virtual,
}

/// Placement layout and its exclusively owned retained position.
#[derive(Debug, Default)]
pub struct Placement {
    /// Screen position or placeholder prototype.
    pub location: Location,
    /// Horizontal pixel offset inside the origin cell.
    pub x_offset: u32,
    /// Vertical pixel offset inside the origin cell.
    pub y_offset: u32,
    /// Source rectangle's left pixel.
    pub source_x: u32,
    /// Source rectangle's top pixel.
    pub source_y: u32,
    /// Requested source width; zero uses the image width.
    pub source_width: u32,
    /// Requested source height; zero uses the image height.
    pub source_height: u32,
    /// Requested cell width; zero preserves native size or aspect ratio.
    pub columns: u32,
    /// Requested cell height; zero preserves native size or aspect ratio.
    pub rows: u32,
    /// Display stacking order.
    pub z: i32,
}

impl Placement {
    /// Release a physical origin through the page list that owns it.
    fn deinit(&self, pages: &mut PageList) {
        if let Location::Pin(pin) = self.location {
            pages.untrack_pin(pin);
        }
    }
}

/// Failure to admit an image or placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    /// The image alone exceeds the screen's byte limit.
    StorageLimit,
    /// A placement refers to an image not present in this screen.
    MissingImage,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for StorageError {}

/// Images and placements belonging to one terminal screen.
///
/// Content mutations receive unique process-wide generations. Geometry-only
/// changes set `dirty` without changing the generation. Stored image generations
/// identify their payload independently of later placement mutations.
pub struct ImageStorage {
    /// Whether content or retained geometry needs a renderer refresh.
    pub dirty: bool,
    /// Last content mutation, or zero for never-mutated storage.
    pub generation: u64,
    /// Next automatically allocated image identifier.
    pub next_image_id: u32,
    /// Next anonymous placement identifier.
    next_internal_placement_id: u32,
    /// Owned image payloads keyed by protocol identifier.
    pub images: HashMap<u32, Image>,
    /// Owned placements, including invisible placeholder prototypes.
    pub placements: HashMap<PlacementKey, Placement>,
    /// In-progress chunked transmission.
    pub loading: Option<LoadingImage>,
    /// Allowed local media; direct payloads are always allowed.
    pub image_limits: Limits,
    /// Sum of all stored payload lengths.
    pub total_bytes: usize,
    /// Maximum stored payload bytes; zero disables execution.
    pub total_limit: usize,
}

impl Default for ImageStorage {
    fn default() -> Self {
        Self {
            dirty: false,
            generation: 0,
            next_image_id: 2_147_483_647,
            next_internal_placement_id: 0,
            images: HashMap::new(),
            placements: HashMap::new(),
            loading: None,
            image_limits: Limits::DIRECT,
            total_bytes: 0,
            total_limit: 320 * 1000 * 1000,
        }
    }
}

impl ImageStorage {
    /// Whether the screen accepts image commands.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.total_limit != 0
    }

    /// Record a content mutation, never a geometry-only change.
    pub(super) fn mark_mutated(&mut self) {
        self.dirty = true;
        self.generation = next_generation();
    }

    /// Change the byte quota, evicting oldest unused images first.
    /// Disabling storage also aborts partial uploads and releases all pins.
    pub fn set_limit(&mut self, pages: &mut PageList, limit: usize) {
        if limit == 0 {
            self.clear(pages);
        } else if limit < self.total_bytes {
            self.evict(pages, self.total_bytes - limit);
        }
        self.total_limit = limit;
    }

    /// Clear all content and reset identifiers, retaining media permissions.
    pub fn clear(&mut self, pages: &mut PageList) {
        for (_, placement) in self.placements.drain() {
            placement.deinit(pages);
        }
        let image_limits = self.image_limits;
        *self = Self {
            image_limits,
            ..Self::default()
        };
        self.mark_mutated();
    }

    /// Store a payload, replacing an existing image with the same identifier.
    ///
    /// # Errors
    /// Rejects an image larger than the quota without changing storage.
    pub fn add_image(
        &mut self,
        pages: &mut PageList,
        mut image: Image,
    ) -> Result<(), StorageError> {
        if image.data.len() > self.total_limit {
            return Err(StorageError::StorageLimit);
        }
        let required = self
            .total_bytes
            .saturating_add(image.data.len())
            .saturating_sub(self.total_limit);
        if required > 0 {
            self.evict(pages, required);
        }
        self.mark_mutated();
        image.generation = self.generation;
        self.total_bytes += image.data.len();
        if let Some(old) = self.images.insert(image.id, image) {
            self.total_bytes -= old.data.len();
        }
        Ok(())
    }

    /// Store a placement, replacing an external ID or allocating an anonymous ID.
    /// The placement transfers ownership of its pin, including on error.
    ///
    /// # Errors
    /// Rejects a placement referring to an absent image.
    pub fn add_placement(
        &mut self,
        pages: &mut PageList,
        image_id: u32,
        placement_id: u32,
        placement: Placement,
    ) -> Result<(), StorageError> {
        if !self.images.contains_key(&image_id) {
            placement.deinit(pages);
            return Err(StorageError::MissingImage);
        }
        let placement_id = if placement_id == 0 {
            let id = self.next_internal_placement_id;
            self.next_internal_placement_id = id.wrapping_add(1);
            PlacementId::Internal(id)
        } else {
            PlacementId::External(placement_id)
        };
        let key = PlacementKey {
            image_id,
            placement_id,
        };
        if let Some(old) = self.placements.insert(key, placement) {
            old.deinit(pages);
        }
        self.mark_mutated();
        Ok(())
    }

    /// Resolve an image identifier within this screen.
    #[must_use]
    pub fn image_by_id(&self, id: u32) -> Option<&Image> {
        self.images.get(&id)
    }

    /// Resolve the most recently transmitted image with this number.
    #[must_use]
    pub fn image_by_number(&self, number: u32) -> Option<&Image> {
        self.images
            .values()
            .filter(|image| image.number == number)
            .max_by_key(|image| image.generation)
    }

    /// Release every placement of an image through its owning page list.
    fn remove_placements(&mut self, pages: &mut PageList, image_id: u32) {
        self.placements.retain(|key, placement| {
            if key.image_id != image_id {
                return true;
            }
            placement.deinit(pages);
            false
        });
    }

    /// Evict sufficient bytes, preferring unused images and then older payloads.
    fn evict(&mut self, pages: &mut PageList, required: usize) {
        let mut candidates: Vec<_> = self
            .images
            .values()
            .map(|image| {
                let used = self.placements.keys().any(|key| key.image_id == image.id);
                (used, image.generation, image.id)
            })
            .collect();
        candidates.sort_unstable();
        let mut evicted = 0;
        for (_, _, id) in candidates {
            self.remove_placements(pages, id);
            let Some(image) = self.images.remove(&id) else {
                continue;
            };
            evicted += image.data.len();
            self.total_bytes -= image.data.len();
            self.mark_mutated();
            if evicted >= required {
                break;
            }
        }
    }
}
