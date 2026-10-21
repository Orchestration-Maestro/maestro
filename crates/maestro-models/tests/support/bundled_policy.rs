//! Native controlled-request and response-hook expectations.

use super::REGISTERED_APIS;
/// How one registered protocol treats the caller's cancellation signal and response hook.
pub struct ProtocolPolicy {
    /// Registered protocol identifier.
    pub api: &'static str,
    /// The request uses the signal carried by the payload the payload hook returns.
    pub selected_signal: bool,
    /// The response hook observes each HTTP response before its body is read.
    pub response_hook: bool,
}

/// Every bundled protocol in registration order, with its test policy.
pub const PROTOCOL_POLICIES: [ProtocolPolicy; 9] = [
    ProtocolPolicy {
        api: REGISTERED_APIS[0],
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[1],
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[2],
        selected_signal: false,
        response_hook: false,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[3],
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[4],
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[5],
        selected_signal: false,
        response_hook: true,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[6],
        selected_signal: true,
        response_hook: false,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[7],
        selected_signal: true,
        response_hook: false,
    },
    ProtocolPolicy {
        api: REGISTERED_APIS[8],
        selected_signal: false,
        response_hook: true,
    },
];

/// The test policy of `api`, looked up before any request starts.
///
/// # Panics
///
/// Panics with the protocol's name when the inventory has no entry for it.
#[cfg(test)]
pub fn policy(api: &str) -> &'static ProtocolPolicy {
    PROTOCOL_POLICIES
        .iter()
        .find(|policy| policy.api == api)
        .unwrap_or_else(|| panic!("no protocol test policy for {api}"))
}
