//! An extension contributing resources beside its installed component.
// maestro-component: MAESTRO_TEST_RESOURCE_COMPONENT
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
    api: maestro_extensions_guest::ExtensionAPI,
) -> maestro_extensions_guest::ExtensionFuture<'static, ()> {
    resource::factory(api, "one")
}
#[cfg(target_arch = "wasm32")]
maestro_extensions_guest::export_extension!(factory);
