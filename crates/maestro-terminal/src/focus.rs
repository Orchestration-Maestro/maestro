//! Focus reports sent to a terminal application.

/// A change in terminal focus; callers decide whether reporting is enabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The terminal gained focus.
    Gained,
    /// The terminal lost focus.
    Lost,
}

/// Encode a focus report as CSI I or CSI O, without allocation.
#[must_use]
pub const fn encode(event: Event) -> &'static [u8; 3] {
    match event {
        Event::Gained => b"\x1b[I",
        Event::Lost => b"\x1b[O",
    }
}
