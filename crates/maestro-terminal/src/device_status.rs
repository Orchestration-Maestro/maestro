//! Typed device-status requests.

/// Supported status query, including its protocol namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Operating condition.
    OperatingStatus,
    /// Current cursor coordinates.
    CursorPosition,
    /// Current light or dark color scheme.
    ColorScheme,
}

/// Resolve a request number in the plain or question-mark namespace.
#[must_use]
pub fn req_from_int(value: u16, question: bool) -> Option<Request> {
    match (value, question) {
        (5, false) => Some(Request::OperatingStatus),
        (6, false) => Some(Request::CursorPosition),
        (996, true) => Some(Request::ColorScheme),
        _ => None,
    }
}
