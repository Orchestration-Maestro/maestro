//! An extension contributing resources beside its installed component.
// Tests receive this component's path in MAESTRO_RESOURCE_COMPONENT.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
#[path = "resource_component/factory.rs"]
mod resource;
/// Register the first resource collection.
#[must_use]
pub fn factory(
    api: maestro_extensions_wasm::ExtensionAPI,
) -> maestro_extensions_wasm::ExtensionFuture<'static, ()> {
    resource::factory(api, "one")
}
#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
