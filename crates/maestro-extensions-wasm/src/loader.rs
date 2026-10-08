//! Factory activation and the macro that exports an extension as a component.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::types::{ExtensionAPI, ExtensionFuture};

/// An extension factory: registers callbacks through the API it receives.
pub type ExtensionFactory = Box<dyn FnOnce(ExtensionAPI) -> ExtensionFuture<'static, ()>>;

/// An extension the host activates once. Activation is the host's: it calls the `start`
/// export, which runs [`Extension::load`] and resolves when that has completed. Each
/// registration reaches the host through its imports as `load` makes it.
pub trait Extension {
    /// Registers callbacks through `api`.
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()>;
}

/// Runs a factory against an API and resolves when the factory has completed.
///
/// # Errors
/// Returns the message the factory failed with.
#[must_use]
pub fn load_extension_from_factory(
    factory: ExtensionFactory,
    api: ExtensionAPI,
) -> ExtensionFuture<'static, ()> {
    factory(api)
}

/// Exports an extension factory as the component's entry point.
///
/// The factory is a function `fn(ExtensionAPI) -> ExtensionFuture<'static, ()>`. The
/// expansion defines the exported type and invokes the generated export macro, so the
/// calling crate needs no handles, identities or raw bindings. The glue it names exists only
/// when building for `wasm32`.
#[macro_export]
macro_rules! export_extension {
    ($factory:path) => {
        /// Entry point of the exported extension.
        struct MaestroExtension;

        impl $crate::Extension for MaestroExtension {
            fn load(api: $crate::ExtensionAPI) -> $crate::ExtensionFuture<'static, ()> {
                $factory(api)
            }
        }

        /// Adapter serving the generated exports for the extension.
        type MaestroGlue = $crate::Glue<MaestroExtension>;

        $crate::bindings::export!(MaestroGlue with_types_in $crate::bindings);
    };
}
