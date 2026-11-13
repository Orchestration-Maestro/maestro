//! Opt-in helpers for controlled tests: a session over the scripted provider, session
//! runtimes over it, export fixtures and the task-set runners session tests run on.
//!
//! A helper reports a setup step that failed to its caller, which decides how the test ends.

mod answers;
mod events;
mod export_fixtures;
mod extensions;
mod harness;
mod records;
mod resource_loader;
mod runtime_fixtures;
mod runtimes;
mod scripted_stream;
mod shell;

pub use answers::{hold_first_answer, script_responses};
pub use events::{recorded_number, session_event_view};
pub use export_fixtures::{Recording, environment, published};
pub use extensions::{command_extension, input_extension};
pub use harness::{Auth, Harness, HarnessOptions, harness};
pub use records::{
    Definition, TS, answer, assistant, message_text, template, tool_definition, usage,
};
pub use resource_loader::{
    TestResourceLoader, create_test_extensions_result, create_test_resource_loader,
};
pub use runtime_fixtures::{
    Recorded, Resources, RuntimeOptions, factory, runtime_for, runtime_with, stored_in,
};
pub use runtimes::{on_paused_task_set, on_task_set};
pub use scripted_stream::{Scripted, ScriptedRequests};
pub use shell::{ScriptedShell, ShellFailure};

use serde_json::Value;

/// `view` with `value` stored under `key`; a `view` that is not an object is returned as it is.
fn with_field(mut view: Value, key: &str, value: Value) -> Value {
    if let Some(fields) = view.as_object_mut() {
        fields.insert(key.to_owned(), value);
    }
    view
}
