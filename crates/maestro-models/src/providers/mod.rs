//! Model protocol adapters, the opt-in simulator and the transport they share.

pub mod assistant_output;
pub(crate) mod base64_text;
pub mod chat;
pub mod cloud;
pub mod content;
pub mod faux;
pub mod http;
pub(crate) mod json_text;
pub mod messages;
pub(crate) mod nullable;
pub(crate) mod openai_prompt_cache;
pub mod reasoning;
pub mod responses;
pub(crate) mod selected_fields;

pub mod response_sessions;

pub mod websocket;
