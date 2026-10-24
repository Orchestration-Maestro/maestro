//! Pure workspace topology and caller-owned spawn transactions.

/// Per-key metadata and sequence admission.
pub mod metadata_tokens;

/// Pane geometry, split trees and directional selection.
pub mod tiling;

/// Per-pane viewport state.
pub mod pane_state;
/// Workspace records and the topology owner.
pub mod spaces;
/// Tab topology and caller-owned spawn records.
pub mod tab;
/// Opaque terminal identity.
pub mod terminal_id;

/// Supplied-path topology and screen-history persistence.
pub mod persist;
