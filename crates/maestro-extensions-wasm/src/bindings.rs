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
    additional_derives: [PartialEq],
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
            "maestro:extension/host.callback": crate::tests::host::Identity,
            "maestro:extension/host.abort-signal": crate::tests::host::Flag,
            "maestro:extension/host.tool-update": crate::tests::host::Progress,
            "maestro:extension/host.callback-emitter": crate::tests::host::Emitter,
            "maestro:extension/host.ui-context": crate::tests::host::Ui,
            "maestro:extension/host.theme": crate::tests::host::ThemeHandle,
            "maestro:extension/host.model-registry": crate::tests::host::Catalog,
            "maestro:extension/host.session-tree-node": crate::tests::host::Node,
            "maestro:extension/host.readonly-session-manager": crate::tests::host::Reader,
            "maestro:extension/host.session-manager": crate::tests::host::Writer,
            "maestro:extension/host.event-bus": crate::tests::host::Bus,
            "maestro:extension/host.subscription": crate::tests::host::Subscribed,
            "maestro:extension/host.context": crate::tests::host::Ordinary,
            "maestro:extension/host.command-context": crate::tests::host::Command,
            "maestro:extension/host.replaced-session-context": crate::tests::host::Replaced,
        },
    });
}
