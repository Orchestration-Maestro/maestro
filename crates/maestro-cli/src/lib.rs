//! Command-line input preparation and account commands.
//!
//! The crate turns authored arguments into retained input, prepares file and image
//! attachments, assembles the first message, renders model tables and runs the
//! configuration and session presentation lifecycles through supplied presenters. It
//! also presents the standalone account command's help, provider lists and login.

pub mod input;

#[cfg(not(target_arch = "wasm32"))]
pub mod model_login;
pub mod packages;
