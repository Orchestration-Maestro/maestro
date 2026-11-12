//! Incremental host graphics updates for layered panes.
//!
//! One call emits at most one transaction and reports whether more work
//! remains, so a caller with a transport budget can spread a frame over
//! several writes without ever painting a blank intermediate frame.

use std::collections::HashSet;

use super::super::{
    ClippedPlacement, EncodedGraphics, HostGraphicsCache, HostPlacement, HostSourceKey,
    clipped_placement,
    encode::{
        encode_delete_image, encode_delete_placement, encode_display_placement,
        encode_transmit_and_display, encode_upload_image,
    },
    identity::{host_placement_id, image_signature, kitty_format_code, placement_signature},
};
use super::{
    host_identity, host_image_of, image_transaction_fits, placement_identity, release_source,
};

impl HostGraphicsCache {
    /// Commands that bring the host in line with the visible pane placements.
    #[must_use]
    pub fn update_layers(
        &mut self,
        placements: &[HostPlacement],
        live_pane_sources: &HashSet<HostSourceKey>,
        transaction_budget: Option<usize>,
    ) -> EncodedGraphics {
        let desired = Desired::new(placements);
        let start = self.continuation_index(placements);
        let mut pass = Pass {
            cache: self,
            bytes: Vec::new(),
            emitted: false,
        };
        if pass.release_dead_sources(live_pane_sources) {
            return pass.encoded(true);
        }
        pass.cache
            .retain_desired(live_pane_sources, &desired.sources);
        if pass.delete_stale_placements(&desired.placements) {
            return pass.encoded(true);
        }
        if pass.emit_next(placements, start, transaction_budget) {
            return pass.encoded(true);
        }
        pass.cache.replay_placements = false;
        pass.cache.replayed_placements.clear();
        pass.encoded(false)
    }

    /// Commands that make the host show one placement, or none when it already
    /// shows it, when it is fully clipped away or when it carries no pixels.
    #[must_use]
    fn encode_placement_update(&mut self, placement: &HostPlacement) -> Option<Vec<u8>> {
        let (clipped, format_code) = clipped_placement(placement)?;
        let key @ (host_id, placement_id) = host_identity(placement);
        let image = image_signature(placement, format_code);
        let shown =
            placement_signature(clipped, placement.placement.z, placement.scrollback_offset);
        let image_current = self.images.get(&host_id) == Some(&image);
        let shown_current = self.placements.get(&key) == Some(&shown)
            && (!self.replay_placements || self.replayed_placements.contains(&key));
        if image_current
            && shown_current
            && self.sources.get(&placement.source_key) == Some(&host_id)
        {
            return None;
        }

        let mut bytes = Vec::new();
        let mut displayed = false;
        if !image_current {
            displayed = self.transmit(&mut bytes, placement, key, clipped)?;
            self.images.insert(host_id, image);
        }
        release_source(&mut bytes, self, placement.source_key.clone(), host_id);
        if !displayed && !shown_current {
            encode_display_placement(
                &mut bytes,
                clipped,
                host_id,
                placement_id,
                placement.placement.z,
            );
        }
        self.placements.insert(key, shown);
        if self.replay_placements {
            self.replayed_placements.insert(key);
        }
        Some(bytes)
    }

    /// Transmit the pixels of a changed image, reporting whether it also displayed them.
    fn transmit(
        &mut self,
        bytes: &mut Vec<u8>,
        placement: &HostPlacement,
        key: (u32, u32),
        clipped: ClippedPlacement,
    ) -> Option<bool> {
        let (host_id, placement_id) = key;
        let format_code = kitty_format_code(placement.placement.format);
        if self.images.contains_key(&host_id) {
            if matches!(placement.source_key, HostSourceKey::PaneLayer { .. }) {
                return encode_transmit_and_display(
                    bytes,
                    placement,
                    clipped,
                    host_id,
                    placement_id,
                )
                .then_some(true);
            }
            encode_delete_image(bytes, host_id);
            self.placements.retain(|(id, _), _| *id != host_id);
            self.replayed_placements.retain(|(id, _)| *id != host_id);
        }
        encode_upload_image(bytes, placement, format_code, host_id).then_some(false)
    }

    /// Resume index of the placement scan.
    fn continuation_index(&self, placements: &[HostPlacement]) -> usize {
        self.continuation
            .as_ref()
            .and_then(|(source, id, _)| {
                placements
                    .iter()
                    .position(|placement| {
                        placement.source_key == *source
                            && host_placement_id(&placement.source_key, &placement.placement) == *id
                    })
                    .map(|index| index + 1)
            })
            .or_else(|| self.continuation.as_ref().map(|cursor| cursor.2))
            .map_or(0, |index| index % placements.len().max(1))
    }

    /// Keep only the sources and quarantines the live layers and the frame still use.
    fn retain_desired(
        &mut self,
        live_pane_sources: &HashSet<HostSourceKey>,
        desired: &HashSet<&HostSourceKey>,
    ) {
        self.sources.retain(|source, _| {
            matches!(source, HostSourceKey::PaneLayer { .. }) || desired.contains(source)
        });
        self.oversized
            .retain(|source, _| live_pane_sources.contains(source) || desired.contains(source));
    }
}

/// Sources and placements one frame asks the host to show.
struct Desired<'a> {
    /// Every source the visible placements belong to.
    sources: HashSet<&'a HostSourceKey>,
    /// Every host image and placement identity the visible placements use.
    placements: HashSet<(u32, u32)>,
}

impl<'a> Desired<'a> {
    /// Collect the identities one frame is expected to show.
    fn new(placements: &'a [HostPlacement]) -> Self {
        let sources = placements
            .iter()
            .map(|placement| &placement.source_key)
            .collect();
        let placements = placements
            .iter()
            .filter(|placement| clipped_placement(placement).is_some())
            .map(host_identity)
            .collect();
        Self {
            sources,
            placements,
        }
    }
}

/// One incremental pass over the retained host state.
struct Pass<'a> {
    /// Retained host state the pass edits.
    cache: &'a mut HostGraphicsCache,
    /// Commands emitted so far.
    bytes: Vec<u8>,
    /// Whether a transaction was already emitted.
    emitted: bool,
}

impl Pass<'_> {
    /// Release images of layers that left the live set.
    fn release_dead_sources(&mut self, live_pane_sources: &HashSet<HostSourceKey>) -> bool {
        let mut dead = self
            .cache
            .sources
            .keys()
            .filter(|source| {
                matches!(source, HostSourceKey::PaneLayer { .. })
                    && !live_pane_sources.contains(*source)
            })
            .cloned()
            .collect::<Vec<_>>();
        dead.sort_by(|a, b| a.order().cmp(&b.order()));
        for source in dead {
            let Some(&host_id) = self.cache.sources.get(&source) else {
                continue;
            };
            let last_reference = !self
                .cache
                .sources
                .iter()
                .any(|(other, id)| *other != source && *id == host_id);
            if self.emitted && last_reference {
                return true;
            }
            self.cache.sources.remove(&source);
            if last_reference {
                encode_delete_image(&mut self.bytes, host_id);
                self.cache.images.remove(&host_id);
                self.cache.placements.retain(|(id, _), _| *id != host_id);
                self.cache
                    .replayed_placements
                    .retain(|(id, _)| *id != host_id);
                self.emitted = true;
            }
        }
        false
    }

    /// Delete placements the frame no longer shows.
    fn delete_stale_placements(&mut self, desired: &HashSet<(u32, u32)>) -> bool {
        let mut stale = self
            .cache
            .placements
            .keys()
            .filter(|key| !desired.contains(key))
            .copied()
            .collect::<Vec<_>>();
        stale.sort_unstable();
        for key @ (host_id, placement_id) in stale {
            if self.emitted {
                return true;
            }
            encode_delete_placement(&mut self.bytes, host_id, placement_id);
            self.cache.placements.remove(&key);
            self.cache.replayed_placements.remove(&key);
            self.emitted = true;
        }
        false
    }

    /// Emit the next transaction, reporting whether another call is needed.
    fn emit_next(
        &mut self,
        placements: &[HostPlacement],
        start: usize,
        transaction_budget: Option<usize>,
    ) -> bool {
        for offset in 0..placements.len() {
            let index = (start + offset) % placements.len();
            let placement = &placements[index];
            let signature =
                image_signature(placement, kitty_format_code(placement.placement.format));
            if transaction_budget.is_some()
                && self.cache.oversized.get(&placement.source_key) == Some(&signature)
            {
                continue;
            }
            self.cache.oversized.remove(&placement.source_key);
            let host_id = host_image_of(placement);
            if self.cache.images.get(&host_id) != Some(&signature)
                && !image_transaction_fits(placement, transaction_budget)
            {
                self.cache
                    .oversized
                    .insert(placement.source_key.clone(), signature);
                continue;
            }
            let mut candidate = self.cache.clone();
            let Some(transaction) = candidate.encode_placement_update(placement) else {
                continue;
            };
            if transaction.is_empty() {
                *self.cache = candidate;
                continue;
            }
            if self.emitted {
                return true;
            }
            *self.cache = candidate;
            let (source, id) = placement_identity(placement);
            self.cache.continuation = Some((source, id, (index + 1) % placements.len()));
            self.bytes = transaction;
            self.emitted = true;
        }
        false
    }

    /// Encoded result of the pass.
    fn encoded(self, incomplete: bool) -> EncodedGraphics {
        EncodedGraphics {
            bytes: self.bytes,
            incomplete,
        }
    }
}
