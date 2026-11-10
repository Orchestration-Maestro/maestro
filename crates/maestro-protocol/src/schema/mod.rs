//! Mod protocol records.
//! Adapted for Maestro: names, module layout and Rust documentation changed.

/// Actions.
pub mod actions;
/// Agents.
pub mod agents;
/// Common.
pub mod common;
/// Server.
pub mod endpoint;
/// Event payload records.
mod event_data;
/// Integrations.
pub mod integrations;
/// Events.
pub mod notifications;
/// Pane graphics records.
mod pane_graphics;
/// Pane results.
mod pane_results;
/// Panes.
pub mod panes;
/// Response.
pub mod replies;
/// Session.
pub mod snapshots;
/// Tabs.
pub mod tabs;
/// Workspaces.
pub mod workspaces;
/// Worktrees.
pub mod worktrees;

pub use actions::*;
pub use agents::*;
pub use common::*;
pub use endpoint::*;
pub use integrations::*;
pub use notifications::*;
pub use panes::*;
pub use replies::*;
pub use snapshots::*;
pub use tabs::*;
pub use workspaces::*;
pub use worktrees::*;

pub use crate::rpc_types::{Request, RpcCommand, RpcResponse};

/// Retain compact string enums when Rust variant documentation is present.
pub(crate) fn string_enum(schema: &mut schemars::Schema) {
    let is_string_enum = schema
        .get("oneOf")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|variants| {
            variants.iter().all(|variant| {
                variant.as_object().is_some_and(|object| {
                    object.len() == 2
                        && object.get("type").and_then(serde_json::Value::as_str) == Some("string")
                        && object
                            .get("const")
                            .is_some_and(serde_json::Value::is_string)
                })
            })
        });
    if !is_string_enum {
        return;
    }
    if let Some(serde_json::Value::Array(variants)) = schema
        .as_object_mut()
        .and_then(|object| object.shift_remove("oneOf"))
    {
        let values = variants
            .into_iter()
            .filter_map(|mut variant| variant.as_object_mut()?.shift_remove("const"))
            .collect();
        schema.insert("type".into(), serde_json::json!("string"));
        schema.insert("enum".into(), serde_json::Value::Array(values));
    }
}
