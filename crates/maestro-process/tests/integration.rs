//! Process integration tests compiled as one program.
#![cfg(test)]

mod capture_lifecycle;
mod child_status_checks;
mod detached_server;
mod helper_launches;
mod interactive_pipes;
mod pty_children;
mod raw_children;
mod startup_environment;
mod streamed_capture;
mod synchronous_capture;
mod termination;
mod windows_children;
