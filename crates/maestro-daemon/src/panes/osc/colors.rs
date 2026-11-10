//! Default-colour policy observations with current-write byte offsets.

use super::collector::OscStreamCollector;

/// Dynamic colour selected by the child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panes) enum DefaultColorQuery {
    /// Text foreground.
    Foreground,
    /// Text background.
    Background,
    /// Cursor colour.
    Cursor,
}

impl DefaultColorQuery {
    /// OSC command selecting this colour.
    pub(in crate::panes) fn osc_number(self) -> u8 {
        match self {
            Self::Foreground => 10,
            Self::Background => 11,
            Self::Cursor => 12,
        }
    }
}

/// Host-colour policy to apply after the engine consumes the corresponding bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panes) enum DefaultColorEvent {
    /// Request the effective dynamic colour.
    Query(DefaultColorQuery),
    /// Record child ownership of a dynamic colour.
    Set(DefaultColorQuery),
    /// Return a dynamic colour to its cached host default.
    Reset(DefaultColorQuery),
    /// Request an effective palette entry.
    PaletteQuery(u8),
}

/// Policy event at its exclusive end offset in the current input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::panes) struct DefaultColorTrackedEvent {
    /// Current-write byte offset after the terminator.
    pub(in crate::panes) end_offset: usize,
    /// Policy observation.
    pub(in crate::panes) event: DefaultColorEvent,
}

/// Bounded framing shared with other OSC consumers, without colour mutation.
#[derive(Debug)]
pub(in crate::panes) struct DefaultColorEventTracker {
    /// OSC framing and incomplete body.
    collector: OscStreamCollector,
    /// Completed policy events.
    pending: Vec<DefaultColorTrackedEvent>,
}

impl Default for DefaultColorEventTracker {
    fn default() -> Self {
        Self {
            collector: OscStreamCollector::new(1024),
            pending: Vec::new(),
        }
    }
}

impl DefaultColorEventTracker {
    /// Observe policy events; the engine remains the colour mutation owner.
    pub(in crate::panes) fn observe(&mut self, bytes: &[u8]) {
        self.collector.observe(bytes, |body, end_offset| {
            self.pending.extend(
                parse_default_color_events(body)
                    .into_iter()
                    .map(|event| DefaultColorTrackedEvent { end_offset, event }),
            );
        });
    }

    /// One accepted event whose terminator is not complete yet.
    pub(in crate::panes) fn in_progress_event(&self) -> Option<DefaultColorEvent> {
        let events = parse_default_color_events(self.collector.in_progress_body()?);
        (events.len() == 1).then(|| events[0])
    }

    /// Take completed events in input order.
    pub(in crate::panes) fn drain_pending(&mut self) -> Vec<DefaultColorTrackedEvent> {
        std::mem::take(&mut self.pending)
    }
}

/// Decode the pane policy events carried by one complete OSC body.
fn parse_default_color_events(body: &[u8]) -> Vec<DefaultColorEvent> {
    let single = match body {
        b"10;?" => Some(DefaultColorEvent::Query(DefaultColorQuery::Foreground)),
        b"11;?" => Some(DefaultColorEvent::Query(DefaultColorQuery::Background)),
        b"12;?" => Some(DefaultColorEvent::Query(DefaultColorQuery::Cursor)),
        b"110" | b"110;" => Some(DefaultColorEvent::Reset(DefaultColorQuery::Foreground)),
        b"111" | b"111;" => Some(DefaultColorEvent::Reset(DefaultColorQuery::Background)),
        _ => parse_palette_color_query(body),
    };
    if let Some(event) = single {
        return vec![event];
    }
    parse_default_color_set_events(body)
}

/// Accept one bounded decimal palette index.
fn parse_palette_color_query(body: &[u8]) -> Option<DefaultColorEvent> {
    let index = body.strip_prefix(b"4;")?.strip_suffix(b";?")?;
    if index.is_empty() || index.len() > 3 || !index.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let mut value: u16 = 0;
    for &digit in index {
        value = value * 10 + u16::from(digit - b'0');
    }
    u8::try_from(value)
        .ok()
        .map(DefaultColorEvent::PaletteQuery)
}

/// Retain each nonempty multi-colour assignment in source order.
fn parse_default_color_set_events(body: &[u8]) -> Vec<DefaultColorEvent> {
    let Some(separator) = body.iter().position(|byte| *byte == b';') else {
        return Vec::new();
    };
    let start = match &body[..separator] {
        b"10" => 10,
        b"11" => 11,
        b"12" => 12,
        _ => return Vec::new(),
    };
    body[separator + 1..]
        .split(|byte| *byte == b';')
        .filter(|value| !value.is_empty())
        .enumerate()
        .filter_map(|(offset, value)| {
            if value == b"?" {
                return None;
            }
            let query = match start + offset {
                10 => DefaultColorQuery::Foreground,
                11 => DefaultColorQuery::Background,
                12 => DefaultColorQuery::Cursor,
                _ => return None,
            };
            Some(DefaultColorEvent::Set(query))
        })
        .collect()
}

#[cfg(test)]
mod tests;
