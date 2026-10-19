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

/// Caller-selected terminal colour scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    /// Light background.
    Light,
    /// Dark background.
    Dark,
}

impl ColorScheme {
    /// Encode the colour-scheme status report.
    ///
    /// # Errors
    /// Returns the writer's original error on failure.
    pub fn encode<W: std::io::Write>(self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(match self {
            Self::Dark => b"\x1b[?997;1n",
            Self::Light => b"\x1b[?997;2n",
        })
    }
}

impl crate::Terminal {
    /// Set the scheme used to answer colour-scheme queries; `None` suppresses replies.
    ///
    /// A change between two known schemes also queues a report when mode 2031
    /// is enabled. Initial discovery, removal and unchanged values do not.
    pub fn set_color_scheme(&mut self, scheme: Option<ColorScheme>) {
        let previous = self.color_scheme;
        self.color_scheme = scheme;
        if let (Some(previous), Some(current)) = (previous, scheme)
            && previous != current
            && self.modes.get(crate::Mode::ReportColorScheme)
        {
            let _ = current.encode(&mut self.effects.pty_reply);
        }
    }
}
