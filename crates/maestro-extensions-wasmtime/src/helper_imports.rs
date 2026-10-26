//! Synchronous helper calls delegate to the native domain without retaining guest operands.
use wasmtime::{StoreContextMut, component::LinkerInstance};

/// Installs ordinary helper calls on the canonical host interface.
pub(crate) fn add_to_linker<T: 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "native-helper",
        |_store: StoreContextMut<'_, T>, (operation, data): (String, String)| {
            Ok((maestro_extensions::helpers::call(&operation, &data),))
        },
    )
}
