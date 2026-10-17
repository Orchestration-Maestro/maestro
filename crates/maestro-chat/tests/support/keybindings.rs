//! Shared selector admission and configurable key publication.
use crate::registry;
/// Restores bindings before releasing the transaction admission.
pub struct BindingsGuard {
    /// Restored first, including assertion unwind.
    _restore: registry::Restore,
    /// Admission spans construction, input and cleanup.
    _guard: std::sync::MutexGuard<'static, ()>,
}
/// Admits one complete transaction with process-wide keybindings.
pub fn bindings_guard() -> BindingsGuard {
    let guard = registry::REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    BindingsGuard {
        _restore: registry::Restore(maestro_tui::get_keybindings()),
        _guard: guard,
    }
}
/// Publish replacement bindings for future input calls.
pub fn bindings(keys: [&str; 4]) {
    let overrides = [
        "tui.select.up",
        "tui.select.down",
        "tui.select.confirm",
        "tui.select.cancel",
    ]
    .into_iter()
    .zip(keys)
    .map(|(action, key)| {
        (
            action.into(),
            Some(maestro_tui::KeybindingKeys::Single(key.into())),
        )
    })
    .collect();
    maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
        maestro_tui::TUI_KEYBINDINGS.clone(),
        overrides,
    ));
}
