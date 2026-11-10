//! Shared sequence allocation and bounded event replay.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use maestro_protocol::events::EventData;

/// Shared ordered history of the most recent daemon events.
#[derive(Clone, Default)]
pub struct EventHub {
    /// One sequence and history across every clone.
    inner: Arc<Mutex<EventHubState>>,
}

/// Sequence allocator and retained event payloads.
#[derive(Default)]
struct EventHubState {
    /// Most recently published sequence, zero before the first event.
    next_sequence: u64,
    /// Ordered events retained for subsequent subscribers.
    events: VecDeque<(u64, EventData)>,
}

impl EventHub {
    /// Number of events retained independently of subscriber progress.
    const MAX_EVENTS: usize = 512;

    /// Publishes an event with the next sequence shared by all clones.
    pub fn push(&self, event: EventData) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        state.next_sequence += 1;
        let sequence = state.next_sequence;
        state.events.push_back((sequence, event));
        if state.events.len() > Self::MAX_EVENTS {
            state.events.pop_front();
        }
    }

    /// Returns retained events strictly newer than the supplied sequence.
    #[must_use]
    pub fn events_after(&self, sequence: u64) -> Vec<(u64, EventData)> {
        let Ok(state) = self.inner.lock() else {
            return Vec::new();
        };
        state
            .events
            .iter()
            .filter(|(event_sequence, _)| *event_sequence > sequence)
            .cloned()
            .collect()
    }

    /// Returns the last allocated sequence, including evicted events.
    #[must_use]
    pub fn current_sequence(&self) -> u64 {
        self.inner.lock().map_or(0, |state| state.next_sequence)
    }
}
