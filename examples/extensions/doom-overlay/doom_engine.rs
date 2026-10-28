//! The retained engine imported by the composed extension.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::bindings::maestro::doom::engine;
use maestro_extensions_wasm::ExtensionResult;
use std::cell::Cell;

/// One initialized game; restarting it requires a fresh composed component.
pub struct DoomEngine {
    /// Completed frame-clock advances, retained while the overlay is closed.
    ticks: Cell<u32>,
    /// Whether an engine operation has failed.
    failed: Cell<bool>,
}
impl DoomEngine {
    /// Initializes the engine with supplied bytes and an isolated writable directory.
    /// # Errors
    /// Returns the engine's initialization error.
    pub fn new(wad: &[u8], root: &str) -> ExtensionResult<Self> {
        engine::init(wad, root)?;
        Ok(Self {
            ticks: Cell::new(0),
            failed: Cell::new(false),
        })
    }
    /// Advances the supplied game clock at 35 frames per second.
    /// # Errors
    /// Returns the engine's tick error.
    pub fn tick(&self) -> ExtensionResult<()> {
        let ticks = self.ticks.get().wrapping_add(1);
        let milliseconds = (ticks / 35)
            .wrapping_mul(1000)
            .wrapping_add((ticks % 35) * 1000 / 35);
        engine::tick(milliseconds).inspect_err(|_| self.failed.set(true))?;
        self.ticks.set(ticks);
        Ok(())
    }
    /// Returns a complete RGBA framebuffer, rejecting malformed external output.
    /// # Errors
    /// Returns a framebuffer length mismatch.
    pub fn get_frame_rgba(&self) -> ExtensionResult<Vec<u8>> {
        let frame = engine::frame();
        if frame.len() != 640 * 400 * 4 {
            self.failed.set(true);
            return Err(format!(
                "DOOM returned {} framebuffer bytes; expected 1024000",
                frame.len()
            ));
        }
        Ok(frame)
    }
    /// Whether this component needs replacement after a failed engine operation.
    #[must_use]
    pub fn has_failed(&self) -> bool {
        self.failed.get()
    }

    /// Enqueues a press or release.
    pub fn push_key(&self, pressed: bool, key: u8) {
        if !self.failed.get() {
            engine::key(pressed, key);
        }
    }
}
