//! Shared integration-test executable.
#![cfg(test)]

mod daemon_connection;
mod daemon_socket;
mod extension_ui;
mod rpc_client_events;
mod rpc_client_lifecycle;
mod rpc_client_transport;
#[cfg(unix)]
#[path = "support/rpc_peer.rs"]
mod rpc_peer;
mod server_commands;
mod server_events;
mod server_lifetime;
mod server_signals;
mod session_decisions;
mod support;
mod transports;
mod wire_records;
