//! The login provider registry as one line per provider, in registry order.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

/// Each registered provider as `identifier: display name`, in registry order.
pub(crate) fn listed() -> Vec<String> {
    maestro_models::get_oauth_providers()
        .iter()
        .map(|provider| format!("{}: {}", provider.id(), provider.name()))
        .collect()
}
