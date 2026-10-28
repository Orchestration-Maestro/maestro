//! Executable startup, package commands and session protocol qualification.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod component_stdout;
mod entry_routes;
mod model_streams;
mod native_io;
mod provider_payload_example;
mod qna_example;
mod rpc_demo_example;
mod startup_modes;
mod structured_output_example;
mod support;
mod tic_tac_toe_example;
pub mod tools_example;
mod truncated_tool_example;
mod windows_terminal;
