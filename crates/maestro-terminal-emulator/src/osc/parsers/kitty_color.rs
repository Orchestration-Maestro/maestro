//! Colour-protocol field decoding.

use super::super::{Command, Parser, State, Terminator};
use crate::{
    color::Rgb,
    kitty::color::{Kind, Osc, Request},
};

/// Keep valid fields in order, rejecting any field after the request limit.
pub(in crate::osc) fn parse(parser: &mut Parser, terminator_ch: Option<u8>) -> Option<Command> {
    if !parser.alloc {
        parser.state = State::Invalid;
        return None;
    }
    let mut list = Vec::new();
    for field in parser.data().split(|b| *b == b';') {
        if list.len() >= Kind::MAX * 2 {
            return None;
        }
        let equal = field.iter().position(|b| *b == b'=').unwrap_or(field.len());
        let Some(key) = Kind::parse(&field[..equal]) else {
            continue;
        };
        let raw = field.get(equal + 1..).unwrap_or_default();
        let first = raw.iter().position(|b| *b != b' ').unwrap_or(raw.len());
        let end = raw
            .iter()
            .rposition(|b| *b != b' ')
            .map_or(first, |n| n + 1);
        let value = &raw[first..end];
        let request = match value {
            b"" => Request::Reset(key),
            b"?" => Request::Query(key),
            _ => {
                let Ok(color) = Rgb::parse(value) else {
                    continue;
                };
                Request::Set { key, color }
            }
        };
        list.push(request);
    }
    Some(Command::KittyColorProtocol(Osc {
        list,
        terminator: Terminator::init(terminator_ch),
    }))
}
