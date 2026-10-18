//! Pure workspace topology and caller-owned spawn transactions.

/// Per-key metadata and sequence admission.
pub mod metadata_tokens;

/// Pane geometry, split trees and directional selection.
pub mod layout;

/// Per-pane viewport state.
pub mod pane_state;
/// Tab topology and caller-owned spawn records.
pub mod tab;
/// Opaque terminal identity.
pub mod terminal_id;
/// Workspace records and the topology owner.
pub mod workspace;

/// Supplied-path topology and screen-history persistence.
pub mod persist;
