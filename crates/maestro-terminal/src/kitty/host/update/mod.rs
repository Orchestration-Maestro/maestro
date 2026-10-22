//! Incremental host graphics updates.

pub(super) mod layers;

pub(super) mod terminal;

use super::{
    HostGraphicsCache, HostPlacement, HostSourceKey, KITTY_CHUNK_BYTES,
    encode::encode_delete_image, host_placement_id,
};

/// Whether one transmission of the placement fits the transport budget.
pub(super) fn image_transaction_fits(placement: &HostPlacement, budget: Option<usize>) -> bool {
    let Some(budget) = budget else {
        return true;
    };
    let data = placement.placement.data_len;
    let encoded = data.div_ceil(3).saturating_mul(4);
    let command_overhead = data.div_ceil(KITTY_CHUNK_BYTES).saturating_mul(16) + 1024;
    encoded.saturating_add(command_overhead) <= budget
}

/// Host image and placement identity the cache keys an entry by.
pub(super) fn host_identity(placement: &HostPlacement) -> (u32, u32) {
    let host_id = placement
        .host_image_id
        .unwrap_or_else(|| super::host_image_id(placement.pane_id, &placement.placement));
    (
        host_id,
        host_placement_id(&placement.source_key, &placement.placement),
    )
}

/// Source and host placement identity of a visible placement.
pub(super) fn placement_identity(placement: &HostPlacement) -> (HostSourceKey, u32) {
    (
        placement.source_key.clone(),
        host_placement_id(&placement.source_key, &placement.placement),
    )
}

/// Delete a superseded host image unless another source still shows it.
pub(super) fn release_source(
    bytes: &mut Vec<u8>,
    cache: &mut HostGraphicsCache,
    source: HostSourceKey,
    host_id: u32,
) {
    let Some(previous) = cache.sources.insert(source, host_id) else {
        return;
    };
    if previous == host_id || cache.sources.values().any(|id| *id == previous) {
        return;
    }
    encode_delete_image(bytes, previous);
    cache.images.remove(&previous);
    cache.placements.retain(|(id, _), _| *id != previous);
    cache.replayed_placements.retain(|(id, _)| *id != previous);
}
