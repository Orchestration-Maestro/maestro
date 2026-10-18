//! Colour-protocol request keys and owned requests.

use crate::{color::Rgb, osc::Terminator};

/// Named colour-protocol targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    /// The foreground target.
    Foreground,
    /// The background target.
    Background,
    /// The selection foreground target.
    SelectionForeground,
    /// The selection background target.
    SelectionBackground,
    /// The cursor target.
    Cursor,
    /// The cursor text target.
    CursorText,
    /// The visual bell target.
    VisualBell,
    /// The second transparent background target.
    SecondTransparentBackground,
}

impl Special {
    /// Exact protocol spelling.
    const fn name(self) -> &'static str {
        match self {
            Self::Foreground => "foreground",
            Self::Background => "background",
            Self::SelectionForeground => "selection_foreground",
            Self::SelectionBackground => "selection_background",
            Self::Cursor => "cursor",
            Self::CursorText => "cursor_text",
            Self::VisualBell => "visual_bell",
            Self::SecondTransparentBackground => "second_transparent_background",
        }
    }
}

/// Palette or named protocol key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Indexed palette entry.
    Palette(u8),
    /// Named colour target.
    Special(Special),
}

impl Kind {
    /// Protocol key bound used for the request-list limit.
    pub const MAX: usize = 263;

    /// Decode an exact name or an unsigned decimal palette index.
    #[must_use]
    pub fn parse(key: &[u8]) -> Option<Self> {
        const SPECIALS: [Special; 8] = [
            Special::Foreground,
            Special::Background,
            Special::SelectionForeground,
            Special::SelectionBackground,
            Special::Cursor,
            Special::CursorText,
            Special::VisualBell,
            Special::SecondTransparentBackground,
        ];
        if let Some(special) = SPECIALS.into_iter().find(|s| s.name().as_bytes() == key) {
            return Some(Self::Special(special));
        }
        if matches!(key.first(), Some(b'+' | b'-')) {
            return None;
        }
        u8::try_from(crate::color::index(key, 10, 255).ok()?)
            .ok()
            .map(Self::Palette)
    }

    /// Whether a terminal retains queryable state for this key.
    #[must_use]
    pub const fn has_terminal_query_color(self) -> bool {
        matches!(
            self,
            Self::Palette(_)
                | Self::Special(Special::Foreground | Special::Background | Special::Cursor)
        )
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Palette(n) => n.fmt(f),
            Self::Special(s) => f.write_str(s.name()),
        }
    }
}

/// One colour-protocol request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Query the selected colour.
    Query(Kind),
    /// Set the selected colour.
    Set {
        /// Selected key.
        key: Kind,
        /// Assigned colour.
        color: Rgb,
    },
    /// Reset the selected colour.
    Reset(Kind),
}

/// Ordered colour-protocol requests with their reply terminator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Osc {
    /// Every admitted request, including duplicates.
    pub list: Vec<Request>,
    /// Reply terminator.
    pub terminator: Terminator,
}
