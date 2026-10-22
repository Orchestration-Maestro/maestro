//! Retained host graphics state.

use std::collections::{HashMap, HashSet};

use super::{
    HostSourceKey, HostView, ImageSignature, PlacementSignature, encode::encode_delete_image,
};

/// What the host terminal already holds and where it is shown.
#[derive(Clone, Debug, Default)]
pub struct HostGraphicsCache {
    /// Payload signature per uploaded host image.
    pub(super) images: HashMap<u32, ImageSignature>,
    /// Shown signature per host image and placement identity.
    pub(super) placements: HashMap<(u32, u32), PlacementSignature>,
    /// Host image backing each source.
    pub(super) sources: HashMap<HostSourceKey, u32>,
    /// Payload quarantined out of the transport budget, per source.
    pub(super) oversized: HashMap<HostSourceKey, ImageSignature>,
    /// Where the next incremental update resumes.
    pub(super) continuation: Option<(HostSourceKey, u32, usize)>,
    /// View the retained placements belong to.
    pub(super) view: Option<HostView>,
    /// Whether every visible placement must be displayed again.
    pub(super) replay_placements: bool,
    /// Placements already displayed since the replay was requested.
    pub(super) replayed_placements: HashSet<(u32, u32)>,
}

impl HostGraphicsCache {
    /// Record the view the next placements belong to, reporting whether it changed.
    pub fn update_view(&mut self, view: Option<HostView>) -> bool {
        if self.view == view {
            return false;
        }
        self.view = view;
        self.continuation = None;
        true
    }

    /// Ask for every visible placement to be displayed again without its pixels.
    pub fn request_placement_replay(&mut self) {
        if !self.replay_placements {
            self.replay_placements = true;
            self.replayed_placements.clear();
        }
    }

    /// Commands that delete every retained image, leaving no state behind.
    pub fn clear_bytes(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for id in self.images.keys().copied().collect::<Vec<_>>() {
            encode_delete_image(&mut bytes, id);
        }
        self.images.clear();
        self.placements.clear();
        self.sources.clear();
        self.reset_incremental_state();
        self.view = None;
        bytes
    }

    /// Drop the state one incremental pass builds up.
    fn reset_incremental_state(&mut self) {
        self.oversized.clear();
        self.continuation = None;
        self.replay_placements = false;
        self.replayed_placements.clear();
    }
}
