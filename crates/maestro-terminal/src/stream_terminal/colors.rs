//! Apply ordered colour operations and queue their protocol-specific reports.

use std::io::Write;

use super::Handler;
use crate::{
    color::{Dynamic, Rgb},
    kitty::color::{Kind, Osc, Request as KittyRequest, Special},
    osc::{
        Terminator,
        color::{Request, Target},
    },
};

impl Handler<'_> {
    /// Apply operations in order so queries observe preceding mutations.
    pub(super) fn color_operation(&mut self, requests: &[Request], terminator: Terminator) {
        for request in requests {
            match *request {
                Request::Set(value) => self.change_color(value.target, Some(value.color)),
                Request::Reset(target) => self.change_color(target, None),
                Request::ResetPalette => {
                    let t = &mut *self.terminal;
                    t.flags.dirty.palette |=
                        t.colors.palette.mask != crate::color::PaletteMask::default();
                    t.colors.palette.reset_all();
                }
                Request::Query(target) => self.color_report(target, terminator),
                Request::ResetSpecial => {}
            }
        }
    }

    /// Mutate the one retained owner shared by both colour protocols.
    fn change_color(&mut self, target: Target, color: Option<Rgb>) {
        let t = &mut *self.terminal;
        let dynamic = match target {
            Target::Palette(index) => {
                t.flags.dirty.palette = true;
                match color {
                    Some(color) => t.colors.palette.set(index, color),
                    None => t.colors.palette.reset(index),
                }
                return;
            }
            Target::Dynamic(Dynamic::Foreground) => &mut t.colors.foreground,
            Target::Dynamic(Dynamic::Background) => &mut t.colors.background,
            Target::Dynamic(Dynamic::Cursor) => &mut t.colors.cursor,
            _ => return,
        };
        match color {
            Some(color) => dynamic.set(color),
            None => dynamic.reset(),
        }
    }

    /// Encode a supported query using replicated sixteen-bit channels.
    fn color_report(&mut self, target: Target, terminator: Terminator) {
        let Some(color) = self.terminal.color_for_xterm(target) else {
            return;
        };
        let bytes = &mut self.terminal.effects.pty_reply;
        match target {
            Target::Palette(index) => {
                let _ = write!(bytes, "\x1b]4;{index};");
            }
            Target::Dynamic(dynamic) => {
                let _ = write!(bytes, "\x1b]{};", dynamic as u8);
            }
            Target::Special(_) => return,
        }
        let _ = color.encode_rgb16(bytes);
        bytes.extend_from_slice(terminator.string());
    }

    /// Encode all supported keyboard-protocol queries in one OSC reply.
    pub(super) fn kitty_color_operation(&mut self, request: &Osc) {
        let mut response = Vec::new();
        for item in &request.list {
            self.kitty_color_request(*item, &mut response);
        }
        if !response.is_empty() {
            response.extend_from_slice(request.terminator.string());
            self.terminal.effects.pty_reply.extend(response);
        }
    }

    /// Apply one mutation or append one query field to the shared response.
    fn kitty_color_request(&mut self, request: KittyRequest, response: &mut Vec<u8>) {
        match request {
            KittyRequest::Set { key, color } => {
                if let Some(target) = kitty_target(key) {
                    self.change_color(target, Some(color));
                }
            }
            KittyRequest::Reset(key) => {
                if let Some(target) = kitty_target(key) {
                    self.change_color(target, None);
                }
            }
            KittyRequest::Query(key) => {
                if !key.has_terminal_query_color() {
                    return;
                }
                if response.is_empty() {
                    response.extend_from_slice(b"\x1b]21");
                }
                let _ = write!(response, ";{key}=");
                if let Some(color) = self.terminal.color_for_kitty(key) {
                    let _ = color.encode_rgb8(response);
                }
            }
        }
    }
}

/// Map supported keyboard-protocol mutations to the shared retained targets.
fn kitty_target(key: Kind) -> Option<Target> {
    match key {
        Kind::Palette(index) => Some(Target::Palette(index)),
        Kind::Special(Special::Foreground) => Some(Target::Dynamic(Dynamic::Foreground)),
        Kind::Special(Special::Background) => Some(Target::Dynamic(Dynamic::Background)),
        Kind::Special(Special::Cursor) => Some(Target::Dynamic(Dynamic::Cursor)),
        Kind::Special(_) => None,
    }
}
