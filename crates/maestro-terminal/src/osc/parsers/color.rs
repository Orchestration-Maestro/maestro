//! Ordered ANSI and dynamic colour requests.

use super::super::{Command, Parser, State, Terminator};
use crate::color::{Dynamic, Rgb, Special};

/// Colour operations identified by their OSC number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum Operation {
    /// Colour operation 4.
    Osc4 = 4,
    /// Colour operation 5.
    Osc5 = 5,
    /// Colour operation 10.
    Osc10 = 10,
    /// Colour operation 11.
    Osc11 = 11,
    /// Colour operation 12.
    Osc12 = 12,
    /// Colour operation 13.
    Osc13 = 13,
    /// Colour operation 14.
    Osc14 = 14,
    /// Colour operation 15.
    Osc15 = 15,
    /// Colour operation 16.
    Osc16 = 16,
    /// Colour operation 17.
    Osc17 = 17,
    /// Colour operation 18.
    Osc18 = 18,
    /// Colour operation 19.
    Osc19 = 19,
    /// Colour operation 104.
    Osc104 = 104,
    /// Colour operation 105.
    Osc105 = 105,
    /// Colour operation 110.
    Osc110 = 110,
    /// Colour operation 111.
    Osc111 = 111,
    /// Colour operation 112.
    Osc112 = 112,
    /// Colour operation 113.
    Osc113 = 113,
    /// Colour operation 114.
    Osc114 = 114,
    /// Colour operation 115.
    Osc115 = 115,
    /// Colour operation 116.
    Osc116 = 116,
    /// Colour operation 117.
    Osc117 = 117,
    /// Colour operation 118.
    Osc118 = 118,
    /// Colour operation 119.
    Osc119 = 119,
}

/// Owned requests in input order.
pub type List = Vec<Request>;

/// One colour request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Assign a colour.
    Set(ColoredTarget),
    /// Query a colour.
    Query(Target),
    /// Restore one target.
    Reset(Target),
    /// Restore the entire palette.
    ResetPalette,
    /// Restore all special colours.
    ResetSpecial,
}

/// A colour's palette, special or dynamic target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Palette index.
    Palette(u8),
    /// Special text colour.
    Special(Special),
    /// Dynamic terminal colour.
    Dynamic(Dynamic),
}

/// A target with its assigned RGB value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColoredTarget {
    /// Selected colour target.
    pub target: Target,
    /// Assigned colour.
    pub color: Rgb,
}

impl Operation {
    /// Decode an admitted operation number.
    fn from_number(n: u16) -> Option<Self> {
        match n {
            4 => Some(Self::Osc4),
            5 => Some(Self::Osc5),
            10 => Some(Self::Osc10),
            11 => Some(Self::Osc11),
            12 => Some(Self::Osc12),
            13 => Some(Self::Osc13),
            14 => Some(Self::Osc14),
            15 => Some(Self::Osc15),
            16 => Some(Self::Osc16),
            17 => Some(Self::Osc17),
            18 => Some(Self::Osc18),
            19 => Some(Self::Osc19),
            104 => Some(Self::Osc104),
            105 => Some(Self::Osc105),
            110 => Some(Self::Osc110),
            111 => Some(Self::Osc111),
            112 => Some(Self::Osc112),
            113 => Some(Self::Osc113),
            114 => Some(Self::Osc114),
            115 => Some(Self::Osc115),
            116 => Some(Self::Osc116),
            117 => Some(Self::Osc117),
            118 => Some(Self::Osc118),
            119 => Some(Self::Osc119),
            _ => None,
        }
    }
}

/// Decode colour requests while keeping each list's stopping policy.
pub fn parse(parser: &mut Parser, terminator_ch: Option<u8>) -> Option<Command> {
    if !parser.alloc {
        parser.state = State::Invalid;
        return None;
    }
    let (State::Capture(n) | State::Prefix(n)) = parser.state else {
        return None;
    };
    let op = Operation::from_number(n)?;
    let mut tokens = parser
        .data()
        .split(|b| *b == b';')
        .filter(|field| !field.is_empty());
    let requests = match op {
        Operation::Osc4 | Operation::Osc5 => ansi_set(op, &mut tokens),
        Operation::Osc104 | Operation::Osc105 => ansi_reset(op, &mut tokens),
        _ => dynamic_requests(n, &mut tokens),
    };
    Some(Command::ColorOperation {
        op,
        requests,
        terminator: Terminator::init(terminator_ch),
    })
}

/// Select a palette or special target using the operation's namespace.
fn ansi_target(op: Operation, value: &[u8]) -> Option<Target> {
    let n = crate::color::index(value, 10, 511).ok()?;
    let n = if matches!(op, Operation::Osc5 | Operation::Osc105) {
        n
    } else {
        if let Ok(index) = u8::try_from(n) {
            return Some(Target::Palette(index));
        }
        n - 256
    };
    let special = match n {
        0 => Special::Bold,
        1 => Special::Underline,
        2 => Special::Blink,
        3 => Special::Reverse,
        4 => Special::Italic,
        _ => return None,
    };
    Some(Target::Special(special))
}

/// Read pairs once and stop at the first incomplete or invalid pair.
fn ansi_set<'a>(op: Operation, tokens: &mut impl Iterator<Item = &'a [u8]>) -> List {
    let mut requests = Vec::new();
    while let Some(index) = tokens.next() {
        let Some(spec) = tokens.next() else {
            break;
        };
        let Some(target) = ansi_target(op, index) else {
            break;
        };
        let Some(request) = assignment(target, spec) else {
            break;
        };
        requests.push(request);
    }
    requests
}

/// Skip invalid reset fields, falling back to a whole-namespace reset.
fn ansi_reset<'a>(op: Operation, tokens: &mut impl Iterator<Item = &'a [u8]>) -> List {
    let mut requests: List = tokens
        .filter_map(|field| ansi_target(op, field).map(Request::Reset))
        .collect();
    if requests.is_empty() {
        requests.push(if op == Operation::Osc104 {
            Request::ResetPalette
        } else {
            Request::ResetSpecial
        });
    }
    requests
}

/// Decode a query or colour assignment without losing the target.
fn assignment(target: Target, spec: &[u8]) -> Option<Request> {
    if spec == b"?" {
        return Some(Request::Query(target));
    }
    Some(Request::Set(ColoredTarget {
        target,
        color: Rgb::parse(spec).ok()?,
    }))
}

/// Read dynamic targets in their protocol order, or reset one empty list.
fn dynamic_requests<'a>(n: u16, tokens: &mut impl Iterator<Item = &'a [u8]>) -> List {
    const COLORS: [Dynamic; 10] = [
        Dynamic::Foreground,
        Dynamic::Background,
        Dynamic::Cursor,
        Dynamic::PointerForeground,
        Dynamic::PointerBackground,
        Dynamic::TektronixForeground,
        Dynamic::TektronixBackground,
        Dynamic::HighlightBackground,
        Dynamic::TektronixCursor,
        Dynamic::HighlightForeground,
    ];
    let reset = n >= 110;
    let start = usize::from(n - if reset { 110 } else { 10 });
    let mut requests = Vec::new();
    if reset {
        if tokens.next().is_none() {
            requests.push(Request::Reset(Target::Dynamic(COLORS[start])));
        }
    } else {
        for (color, spec) in COLORS[start..].iter().zip(tokens) {
            let Some(request) = assignment(Target::Dynamic(*color), spec) else {
                break;
            };
            requests.push(request);
        }
    }
    requests
}

#[cfg(test)]
mod tests;
