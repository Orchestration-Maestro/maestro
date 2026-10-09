//! Model protocol adapters, the opt-in simulator and the transport they share.

pub(crate) mod assistant_output;
pub mod chat;
pub mod faux;
pub mod http;
pub(crate) mod json_text;
pub mod messages;
pub mod reasoning;
pub mod responses;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "response-session transport awaits its public dispatch caller"
    )
)]
mod response_sessions;
