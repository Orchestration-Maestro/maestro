//! Admission and restoration of the process key registry.
use maestro_tui::{KeybindingsManager, set_keybindings};
#[allow(
    dead_code,
    reason = "Only the toolkit's lock and its guard are reused here."
)]
#[path = "../../../maestro-tui/tests/fixtures/input_support/mod.rs"]
pub(crate) mod input_support;
/// Serializes global key manager replacements across test cases; it is the toolkit input
/// tests' own lock, so tests sharing one executable never replace the bindings concurrently.
pub(crate) use input_support::GLOBALS as REGISTRY;
/// Restores the caller's manager, including after a failed assertion.
pub struct Restore(pub KeybindingsManager);
impl Drop for Restore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
