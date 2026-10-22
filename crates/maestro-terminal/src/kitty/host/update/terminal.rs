//! Host graphics updates for a screen that carries no pane layers.

use std::collections::HashSet;

use super::super::{
    ClippedPlacement, HostGraphicsCache, HostPlacement, HostSourceKey,
    clip::clipped_placement,
    encode::{
        encode_delete_image, encode_delete_placement, encode_display_placement, encode_upload_image,
    },
    identity::{image_signature, placement_signature},
    update::{host_identity, release_source},
};

impl HostGraphicsCache {
    /// Commands that bring the host in line with a screen of terminal images.
    pub fn update_terminal(&mut self, placements: &[HostPlacement], view_changed: bool) -> Vec<u8> {
        let current_sources = placements
            .iter()
            .filter(|placement| matches!(placement.source_key, HostSourceKey::Terminal { .. }))
            .map(|placement| placement.source_key.clone())
            .collect::<HashSet<_>>();
        self.sources
            .retain(|source, _| current_sources.contains(source));

        let mut bytes = Vec::new();
        let mut current = HashSet::new();
        for placement in placements {
            update_placement(&mut bytes, self, placement, &mut current, view_changed);
        }
        let stale = self
            .placements
            .keys()
            .filter(|key| !current.contains(key))
            .copied()
            .collect::<Vec<_>>();
        for (host_id, placement_id) in stale {
            encode_delete_placement(&mut bytes, host_id, placement_id);
            self.placements.remove(&(host_id, placement_id));
        }
        bytes
    }
}

/// Upload, release and display one terminal placement.
fn update_placement(
    bytes: &mut Vec<u8>,
    cache: &mut HostGraphicsCache,
    placement: &HostPlacement,
    current: &mut HashSet<(u32, u32)>,
    view_changed: bool,
) {
    let Some((clipped, format_code)) = clipped_placement(placement) else {
        return;
    };
    let key = host_identity(placement);
    let host_id = key.0;
    let signature = image_signature(placement, format_code);
    current.insert(key);

    match cache.images.get(&host_id).copied() {
        Some(existing) if existing == signature => {}
        Some(_) => {
            encode_delete_image(bytes, host_id);
            cache.placements.retain(|(image, _), _| *image != host_id);
            if !encode_upload_image(bytes, placement, format_code, host_id) {
                return;
            }
            cache.images.insert(host_id, signature);
        }
        None => {
            if !encode_upload_image(bytes, placement, format_code, host_id) {
                return;
            }
            cache.images.insert(host_id, signature);
        }
    }

    release_source(bytes, cache, placement.source_key.clone(), host_id);
    show_placement(bytes, cache, placement, clipped, view_changed);
}

/// Display a placement unless the host already shows it unchanged.
fn show_placement(
    bytes: &mut Vec<u8>,
    cache: &mut HostGraphicsCache,
    placement: &HostPlacement,
    clipped: ClippedPlacement,
    view_changed: bool,
) {
    let signature =
        placement_signature(clipped, placement.placement.z, placement.scrollback_offset);
    let key = host_identity(placement);
    match cache.placements.get_mut(&key) {
        Some(existing) if !view_changed && *existing == signature => {}
        Some(existing) => {
            encode_display_placement(bytes, clipped, key.0, key.1, placement.placement.z);
            *existing = signature;
        }
        None => {
            encode_display_placement(bytes, clipped, key.0, key.1, placement.placement.z);
            cache.placements.insert(key, signature);
        }
    }
}
