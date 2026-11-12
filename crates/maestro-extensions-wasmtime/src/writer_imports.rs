//! Canonical writable setup methods over the live session the local owner retains.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostSessionManager,
    context_imports::add_resource,
    lifecycle::State,
    read_imports::{endpoint, forwards, reads, release},
    writer_reads::Endpoint,
};
use wasmtime::component::{LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

impl HostSessionManager for State {
    reads! { Endpoint;
        set_session_file(path: String) -> ();
        new_session(options: Option<String>) -> Option<String>;
        is_persisted() -> bool;
        get_children(parent_id: String) -> String;
        build_session_context() -> String;
        append_message(message: String) -> String;
        append_thinking_level_change(level: String) -> String;
        append_model_change(provider: String, model_id: String) -> String;
        append_compaction(compaction: String, from_hook: Option<bool>) -> String;
        append_custom_entry(custom_type: String, data: Option<String>) -> String;
        append_session_info(name: String) -> String;
        append_custom_message_entry(message: String) -> String;
        append_label_change(target_id: String, label: Option<String>) -> String;
        branch(from_id: String) -> ();
        reset_leaf() -> ();
        branch_with_summary(
            from_id: Option<String>,
            summary: String,
            metadata: (Option<String>, Option<bool>)
        ) -> String;
        create_branched_session(leaf_id: String) -> Option<String>;
    }

    fn reader(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Resource<crate::history_reads::Endpoint>> {
        let reader = endpoint(self, &this)?
            .reader()
            .map_err(wasmtime::Error::msg)?;
        Ok(self.ctx().table.push(reader)?)
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every writable session method to its trait implementation.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "session-manager")?;
    forwards! { host, "session-manager", HostSessionManager, Endpoint,
        "reader" => reader();
        "set-session-file" => set_session_file(path: String);
        "new-session" => new_session(options: Option<String>);
        "is-persisted" => is_persisted();
        "get-children" => get_children(parent_id: String);
        "build-session-context" => build_session_context();
        "append-message" => append_message(message: String);
        "append-thinking-level-change" => append_thinking_level_change(level: String);
        "append-model-change" => append_model_change(provider: String, model_id: String);
        "append-compaction" => append_compaction(compaction: String, from_hook: Option<bool>);
        "append-custom-entry" => append_custom_entry(custom_type: String, data: Option<String>);
        "append-session-info" => append_session_info(name: String);
        "append-custom-message-entry" => append_custom_message_entry(message: String);
        "append-label-change" => append_label_change(target_id: String, label: Option<String>);
        "branch" => branch(from_id: String);
        "reset-leaf" => reset_leaf();
        "branch-with-summary" => branch_with_summary(
            from_id: Option<String>,
            summary: String,
            metadata: (Option<String>, Option<bool>)
        );
        "create-branched-session" => create_branched_session(leaf_id: String);
    }
    Ok(())
}
