//! Canonical history methods over the live history the local owner retains.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostReadonlySessionManager,
    context_imports::add_resource,
    history_reads::Endpoint,
    lifecycle::State,
    read_imports::{endpoint, forwards, release},
    tree_imports::lower_nodes,
};
use wasmtime::component::{LinkerInstance, Resource};

/// Declares the methods that read one endpoint operation.
macro_rules! reads {
    ($($method:ident($($operand:ident: $kind:ty),*) -> $result:ty;)+) => {
        $(fn $method(
            &mut self,
            this: Resource<Endpoint>,
            $($operand: $kind),*
        ) -> wasmtime::Result<Result<$result, String>> {
            Ok(endpoint(self, &this)?.$method($($operand),*))
        })+
    };
}

impl HostReadonlySessionManager for State {
    reads! {
        get_cwd() -> String;
        get_session_dir() -> String;
        get_session_id() -> String;
        get_session_file() -> Option<String>;
        get_leaf_id() -> Option<String>;
        get_leaf_entry() -> Option<String>;
        get_entry(id: String) -> Option<String>;
        get_label(id: String) -> Option<String>;
        get_branch(from_id: Option<String>) -> String;
        get_header() -> Option<String>;
        get_entries() -> String;
        get_session_name() -> Option<String>;
    }

    fn get_tree(
        &mut self,
        this: Resource<Endpoint>,
    ) -> wasmtime::Result<Result<Vec<Resource<crate::tree_reads::Endpoint>>, String>> {
        let nodes = endpoint(self, &this)?.get_tree();
        lower_nodes(self, nodes)
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every reader method to its trait implementation.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "readonly-session-manager")?;
    forwards! { host, "readonly-session-manager", HostReadonlySessionManager, Endpoint,
        "get-cwd" => get_cwd();
        "get-session-dir" => get_session_dir();
        "get-session-id" => get_session_id();
        "get-session-file" => get_session_file();
        "get-leaf-id" => get_leaf_id();
        "get-leaf-entry" => get_leaf_entry();
        "get-entry" => get_entry(id: String);
        "get-label" => get_label(id: String);
        "get-branch" => get_branch(from_id: Option<String>);
        "get-header" => get_header();
        "get-entries" => get_entries();
        "get-tree" => get_tree();
        "get-session-name" => get_session_name();
    }
    Ok(())
}
