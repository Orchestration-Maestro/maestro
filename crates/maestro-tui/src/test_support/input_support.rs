//! Serialized process input globals with restoration.

use crate::{
    KeybindingsManager, get_keybindings, is_kitty_protocol_active, set_keybindings,
    set_kitty_protocol_active,
};
use std::sync::{Mutex, MutexGuard};

/// Serializes every input test accessing process-wide state.
pub static GLOBALS: Mutex<()> = Mutex::new(());

/// Saved process globals held for one test.
pub struct Globals {
    /// Lock shared by default, custom-binding and protocol tests.
    _lock: MutexGuard<'static, ()>,
    /// Previously active manager.
    bindings: KeybindingsManager,
    /// Previously active keyboard mode.
    protocol: bool,
}

/// Starts with default bindings and restores the previous globals on drop.
pub fn globals() -> Globals {
    let lock = GLOBALS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let guard = Globals {
        _lock: lock,
        bindings: get_keybindings(),
        protocol: is_kitty_protocol_active(),
    };
    set_keybindings(KeybindingsManager::new(
        crate::TUI_KEYBINDINGS.clone(),
        Vec::new(),
    ));
    set_kitty_protocol_active(false);
    guard
}

impl Drop for Globals {
    fn drop(&mut self) {
        set_keybindings(self.bindings.clone());
        set_kitty_protocol_active(self.protocol);
    }
}
