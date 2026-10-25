//! Native component execution through the local extension registration facade.
mod api_imports;
mod api_reads;
mod callback_imports;
mod catalog_imports;
mod catalog_reads;
mod command_imports;
mod command_reads;
mod compact_calls;
mod context_imports;
mod context_reads;
mod effect_imports;
mod event_imports;
mod event_reads;
mod event_registration;
mod factory;
pub use factory::artifact_factory;
mod guest_calls;
mod guest_component;
mod guest_exports;
mod history_imports;
mod history_reads;
mod import_route;
mod lifecycle;
mod local_resources;
mod read_imports;
mod registration_imports;
mod registration_reads;
mod replaced_imports;
mod replaced_reads;
mod schema_imports;
mod schema_node;
mod schema_reads;
mod schema_registration;
mod schema_wire;
mod session_calls;
mod session_imports;
mod theme_imports;
mod theme_reads;
mod tool_registration;
mod tree_imports;
mod tree_reads;
mod ui_imports;
mod ui_reads;
mod writer_imports;
mod writer_reads;

#[cfg(test)]
mod tests;

#[cfg(test)]
use tests::context_actions;
