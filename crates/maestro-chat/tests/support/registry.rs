//! Admission and restoration of the process key registry.
/// Serializes global key manager replacements across test cases; it is the toolkit input
/// tests' own lock, so tests sharing one executable never replace the bindings concurrently.
pub(crate) use input_support::GLOBALS as REGISTRY;
pub(crate) use maestro_tui::test_support::input_support;
use maestro_tui::{KeybindingsManager, set_keybindings};
/// Restores the caller's manager, including after a failed assertion.
pub struct Restore(pub KeybindingsManager);
impl Drop for Restore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
