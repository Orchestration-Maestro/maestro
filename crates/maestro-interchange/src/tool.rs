//! Shared tool interchange records. See the [record guide](../../../docs/tool-records.md).

/// Direct-shell output.
pub mod bash_executor;
/// Built-in tool records.
pub mod tools;
/// Agent tool records and the tool records a guest shares with its host.
pub mod types;
