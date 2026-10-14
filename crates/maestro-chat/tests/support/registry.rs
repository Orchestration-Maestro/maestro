//! Admission and restoration of the process key registry.
use maestro_tui::{KeybindingsManager, set_keybindings};
use std::sync::Mutex;
/// Serializes global key manager replacements across test cases.
pub static REGISTRY: Mutex<()> = Mutex::new(());
/// Restores the caller's manager, including after a failed assertion.
pub struct Restore(pub KeybindingsManager);
impl Drop for Restore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
