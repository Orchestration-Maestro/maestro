//! Overlay facades retain the original host options capability.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::Imports;
use crate::{ExtensionResult, OverlayOptions, OverlayOptionsHandle, tui::OverlayOptionsPort};
use std::rc::Rc;

/// The host and its original shared options record.
pub(super) struct Options<I: Imports> {
    /// Adapter serving this record.
    imports: I,
    /// Original host resource.
    pub(super) resource: I::OverlayOptions,
}
impl<I: Imports> OverlayOptionsPort for Options<I> {
    fn get(&self) -> ExtensionResult<OverlayOptions> {
        let encoded = self.imports.overlay_options_get(&self.resource)?;
        serde_json::from_str(&encoded).map_err(|error| error.to_string())
    }
    fn set(&self, options: OverlayOptions) -> ExtensionResult<()> {
        let encoded = serde_json::to_string(&options).map_err(|error| error.to_string())?;
        self.imports.overlay_options_set(&self.resource, &encoded)
    }
}

/// Creates and retains the host's shared placement record.
pub(super) fn options<I: Imports>(
    imports: &I,
    options: &OverlayOptions,
) -> ExtensionResult<OverlayOptionsHandle> {
    let encoded = serde_json::to_string(&options).map_err(|error| error.to_string())?;
    let resource = imports.new_overlay_options(&encoded)?;
    let owner = Rc::new(Options {
        imports: imports.clone(),
        resource,
    });
    Ok(OverlayOptionsHandle(crate::Supplied::lent(
        owner.clone(),
        owner,
    )))
}
