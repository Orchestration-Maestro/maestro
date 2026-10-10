//! Bindings generated from the canonical interface files in `wit/`.
//!
//! This is the only handwritten file whose lint levels differ from the workspace: the
//! generator emits `#[allow(clippy::all)]` and list-lifting code that repeats one expression
//! as length and capacity, which a `forbid`den lint cannot accept. Cargo lint tables apply
//! to a whole crate, so the crate manifest lowers three groups to `deny` and every
//! handwritten module restores them as `forbid`. The exception is checked by the workspace
//! conventions.
#![allow(clippy::same_length_and_capacity)]

wit_bindgen::generate!({
    path: "wit",
    world: "extension",
    pub_export_macro: true,
    default_bindings_module: "maestro_extensions_wasm::bindings",
    generate_unused_types: true,
});

/// Native host bindings of the same interface files, used by the component tests to drive a
/// built extension from the host side.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod host_side {
    wasmtime::component::bindgen!({
        path: "wit",
        world: "extension",
        imports: { default: trappable },
        with: {
            "maestro:extension/host.tool-update": crate::tests::host::Update,
            "maestro:extension/host.callback": crate::tests::host::Identity,
            "maestro:extension/host.abort-signal": crate::tests::host::Flag,
            "maestro:extension/host.context": crate::tests::host::Ordinary,
            "maestro:extension/host.command-context": crate::tests::host::Command,
            "maestro:extension/host.replaced-session-context": crate::tests::host::Replaced,
        },
    });
}
