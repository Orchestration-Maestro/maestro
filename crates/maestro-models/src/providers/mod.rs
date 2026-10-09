//! Model protocol adapters, the opt-in simulator and the transport they share.

pub(crate) mod assistant_output;
pub mod chat;
pub mod faux;
pub mod http;
pub(crate) mod json_text;
pub mod messages;
pub mod reasoning;
mod responses;
