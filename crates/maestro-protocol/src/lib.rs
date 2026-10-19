//! Shared RPC records and binary screen-channel codecs.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
/// Build info.
pub mod build_info;
/// Events.
pub mod events;
/// Frames.
pub mod frames;
/// Request.
pub mod request;
/// Response.
pub mod response;
/// Schema.
pub mod schema;
/// Shared.
pub mod shared;

/// Shared RPC command and response records.
pub mod rpc_types;

/// Shared command and binary-channel protocol version.
pub const PROTOCOL_VERSION: u32 = 21;

#[cfg(test)]
mod tests;
