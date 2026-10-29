//! Acquisition and integrity of the pinned checkout.

use maestro_extensions_wasm::package_manager::GitPinStatus;

/// What a checkout needs before its files can be read.
#[derive(Debug, PartialEq, Eq)]
pub enum Need {
    /// No checkout exists yet.
    Missing,
    /// The checkout is clean at its pinned commit.
    Clean,
    /// The checkout differs from its pinned commit.
    Drifted,
}

/// Classifies a pin observation.
#[must_use]
pub fn need(status: &GitPinStatus) -> Need {
    if status.is_clean() {
        Need::Clean
    } else if status.expected_commit.is_none() && status.head_commit.is_none() {
        Need::Missing
    } else {
        Need::Drifted
    }
}
