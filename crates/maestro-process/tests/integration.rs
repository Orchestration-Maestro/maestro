//! Process integration tests compiled as one program.
#![cfg(test)]

mod capture_lifecycle;
mod helper_launches;
#[cfg(unix)]
#[path = "support/poll.rs"]
mod poll;
mod pty_children;
mod streamed_capture;
mod synchronous_capture;
