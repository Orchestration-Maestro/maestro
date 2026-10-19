//! Device-attribute request selection.

/// Device-attribute report requested by the input stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Req {
    /// Primary terminal capabilities.
    Primary,
    /// Secondary terminal identification.
    Secondary,
    /// Tertiary terminal identification.
    Tertiary,
}
