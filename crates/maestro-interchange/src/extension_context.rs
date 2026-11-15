//! Option records an extension passes to its context operations: a compaction request, the
//! presentation of a custom component and the continuation of a session switch.
//!
//! The callbacks and the overlay placement differ between a native extension and a guest, so
//! each record takes them as type parameters and each side names its own form through one
//! alias. Every member is optional, whatever those types are, so every record has a default.

/// A compaction request that returns before compaction finishes.
///
/// `C` receives the finished compaction and `E` its failure.
pub struct CompactOptions<C, E> {
    /// Optional literal instructions for the summary.
    pub custom_instructions: Option<String>,
    /// Invoked with the finished compaction.
    pub on_complete: Option<C>,
    /// Invoked with the compaction failure.
    pub on_error: Option<E>,
}

impl<C, E> Default for CompactOptions<C, E> {
    fn default() -> Self {
        Self {
            custom_instructions: None,
            on_complete: None,
            on_error: None,
        }
    }
}

/// Presentation options for a custom component.
///
/// `O` is the overlay placement and `H` receives the overlay's handle.
#[derive(Clone)]
pub struct CustomUIOptions<O, H> {
    /// Whether the component is shown as an overlay rather than replacing the editor.
    pub overlay: Option<bool>,
    /// Overlay positioning and sizing.
    pub overlay_options: Option<O>,
    /// Receives the overlay's handle once the overlay is on screen.
    pub on_handle: Option<H>,
}

impl<O, H> Default for CustomUIOptions<O, H> {
    fn default() -> Self {
        Self {
            overlay: None,
            overlay_options: None,
            on_handle: None,
        }
    }
}

/// Options of a command session switch.
///
/// `W` continues against the replacement session.
pub struct SwitchSessionOptions<W> {
    /// Continuation the host binds to this switch and runs against the replacement session.
    pub with_session: Option<W>,
}

impl<W> Default for SwitchSessionOptions<W> {
    fn default() -> Self {
        Self { with_session: None }
    }
}
