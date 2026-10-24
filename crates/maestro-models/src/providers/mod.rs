//! Model protocol adapters, the opt-in simulator and the transport they share.

pub mod assistant_output;
pub(crate) mod base64_text;
pub mod chat;
pub mod cloud;
pub mod content;
pub(crate) mod json_text;
pub mod messages;
pub(crate) mod nullable;
pub mod reasoning;
pub mod responses_api;
pub mod scripted;
pub(crate) mod selected_fields;
pub mod transport;

pub mod response_sessions;

pub mod websocket;
