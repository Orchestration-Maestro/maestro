//! The manifest base package: reads the `manifest` selection, acquires and verifies the
//! pinned release, reads its areas and publishes the combined settings layer.

/// Area reading and settings combination.
pub mod areas;
/// Load failures that name the settings file, the source, the problem and the fix.
pub mod failure;
/// The settings owner's base-layer record and the load-time payload that carries it.
pub mod layer;
/// Messages shown to the user once a load has finished.
pub mod notice;
/// Acquisition and integrity of the pinned checkout.
pub mod release;
/// The `manifest` setting.
pub mod selection;

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};

/// Selects, acquires and reads the manifest release, then publishes its settings layer.
#[must_use]
pub fn factory(_api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async { Ok(()) })
}

#[cfg(test)]
mod tests;
