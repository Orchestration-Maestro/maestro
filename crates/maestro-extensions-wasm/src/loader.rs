//! Factory activation and the macro that exports an extension as a component.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::types::{ExtensionAPI, ExtensionFuture};

/// An extension factory: registers callbacks through the API it receives.
pub type ExtensionFactory = Box<dyn FnOnce(ExtensionAPI) -> ExtensionFuture<'static, ()>>;

/// An extension the host runs once, awaiting its completion before its registrations are
/// observed.
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
/// calling crate needs no handles, identities or raw bindings.
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
