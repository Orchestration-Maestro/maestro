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
    async: [
        "-import:maestro:extension/host@0.1.0#callback-emit",
        "-import:maestro:extension/host@0.1.0#get-all-tools",
        "-import:maestro:extension/host@0.1.0#register-tool",
    ],
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
            "maestro:extension/host.provider-controls": crate::tests::stream_host::Controls,
            "maestro:extension/host.fs-watcher": crate::tests::native_watch_host::Watcher,
            "maestro:extension/host.response": crate::tests::native_http_host::Response,
            "maestro:extension/host.child-process": crate::tests::native_io_host::Child,
            "maestro:extension/host.readable": crate::tests::native_io_host::Reader,
            "maestro:extension/host.writable": crate::tests::native_io_host::Writer,
            "maestro:extension/host.assistant-stream": crate::tests::stream_host::Assistant,
            "maestro:extension/host.event-stream": crate::tests::stream_host::Stream,
            "maestro:extension/host.stream-value": crate::tests::stream_host::Value,
            "maestro:extension/host.native-row": crate::tests::host::NativeRow,
            "maestro:extension/host.native-component": crate::tests::host::NativeComponent,
            "maestro:extension/host.file-mutation": crate::tests::host::FileMutation,
            "maestro:extension/host.builtin-tool": crate::tests::host::BuiltinTool,
            "maestro:extension/host.local-bash-operations": crate::tests::host::LocalBashOperations,
            "maestro:extension/host.bash-output": crate::tests::shell_host::Receiver,
            "maestro:extension/host.theme": crate::tests::renderer_host::Theme,
            "maestro:extension/host.session-manager": crate::tests::reader_host::Reader,
            "maestro:extension/host.extension-ui-context": crate::tests::ui_host::Ui,
            "maestro:extension/host.auth-lock-update": crate::tests::reader_host::Reader,
            "maestro:extension/host.oauth-provider": crate::tests::reader_host::Reader,
            "maestro:extension/host.oauth-interaction": crate::tests::reader_host::Reader,
            "maestro:extension/host.auth-storage": crate::tests::reader_host::Reader,
            "maestro:extension/host.custom-close": crate::tests::custom_host::Close,
            "maestro:extension/host.overlay-options": crate::tests::overlay_host::Options,
            "maestro:extension/host.overlay-handle": crate::tests::overlay_host::Handle,
            "maestro:extension/host.tui": crate::tests::ui_host::Tui,
            "maestro:extension/host.footer-data": crate::tests::ui_host::FooterResource,
            "maestro:extension/host.tool-invalidator": crate::tests::ui_host::Invalidator,
            "maestro:extension/host.model-registry": crate::tests::reader_host::Reader,
            "maestro:extension/host.readonly-session-manager": crate::tests::reader_host::Reader,
            "maestro:extension/host.session-tree-node": crate::tests::reader_host::Node,
            "maestro:extension/host.subscription": crate::tests::bus_host::Subscription,
            "maestro:extension/host.callback-emitter": crate::tests::bus_host::Emitter,
            "maestro:extension/host.tool-update": crate::tests::host::Update,
            "maestro:extension/host.import-route": crate::tests::host::Route,
            "maestro:extension/host.widget": crate::tests::widget_component::WidgetResource,
            "maestro:extension/host.style": crate::tests::widget_component::StyleResource,
            "maestro:extension/host.highlighter": crate::tests::widget_component::HighlighterResource,
            "maestro:extension/host.row-style": crate::tests::widget_component::RowStyleResource,
            "maestro:extension/host.submenu-done": crate::tests::widget_component::SubmenuDoneResource,
            "maestro:extension/host.keybindings-manager": crate::tests::widget_component::KeybindingsResource,
            "maestro:extension/host.text-callback": crate::tests::widget_component::TextCallbackResource,
            "maestro:extension/host.editor-factory": crate::tests::editor_component::ForeignFactory,
            "maestro:extension/host.completion-provider": crate::tests::provider_host::Lent,
            "maestro:extension/host.local-settlement": crate::tests::host::Settlement,
            "maestro:extension/host.callback": crate::tests::host::Identity,
            "maestro:extension/host.json-node": crate::tests::host::JsonNode,
            "maestro:extension/host.abort-signal": crate::tests::host::Flag,
            "maestro:extension/host.context": crate::tests::host::Ordinary,
            "maestro:extension/host.command-context": crate::tests::host::Command,
            "maestro:extension/host.replaced-session-context": crate::tests::host::Replaced,
        },
    });
}

/// Standard native asynchronous operating-system capabilities supplied by the component host.
pub mod standard {
    wit_bindgen::generate!({
        inline: "package maestro:standard; world standards { include wasi:cli/imports@0.3.0; }",
        path: "wit/standard",
        world: "maestro:standard/standards",
        generate_all,
    });
}
