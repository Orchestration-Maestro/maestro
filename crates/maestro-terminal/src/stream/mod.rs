//! Incremental terminal bytes projected into synchronous typed actions.

use maestro_text::Utf8Stream;

mod action;
mod csi;
mod esc;
mod osc_dispatch;

pub use action::{
    Action, CharsetAction, CursorAction, EditAction, EraseAction, KeyboardAction, ModeAction,
    OscAction, PayloadAction, ProtectionAction, QueryAction, TabAction,
};

/// Synchronous consumer of decoded terminal operations.
pub trait Handler {
    /// Apply one operation before processing the following input.
    fn vt(&mut self, action: Action<'_>);
}

/// Retained byte decoder for successive terminal writes.
pub struct Stream {
    /// Incomplete scalar carried between writes.
    utf8: Utf8Stream,
    /// Existing control-sequence parser with its OSC allocation policy.
    parser: crate::Parser,
}

impl Stream {
    /// Construct a stream with fixed or allocating OSC capture.
    #[must_use]
    pub fn new(allocating: bool) -> Self {
        let mut parser = crate::Parser::init();
        parser.osc_parser.alloc = allocating;
        Self {
            utf8: Utf8Stream::default(),
            parser,
        }
    }

    /// Consume bytes without flushing an incomplete trailing scalar.
    pub fn next_slice(&mut self, bytes: &[u8], handler: &mut impl Handler) {
        let mut remaining = bytes;
        while !remaining.is_empty() {
            if self.parser.state == crate::parser::State::Ground {
                remaining = self.ground(remaining, handler);
            }
            let Some((&byte, rest)) = remaining.split_first() else {
                break;
            };
            for action in self.parser.next(byte).into_iter().flatten() {
                Self::dispatch(action, handler);
            }
            remaining = rest;
        }
    }

    /// Forward each parser action without retaining callback-borrowed views.
    fn dispatch(action: crate::parser::Action<'_>, handler: &mut impl Handler) {
        match action {
            crate::parser::Action::EscDispatch(input) => esc::dispatch(&input, handler),
            crate::parser::Action::CsiDispatch(input) => csi::dispatch(&input, handler),
            crate::parser::Action::Execute(byte) => Self::execute(byte, handler),
            crate::parser::Action::Print(cp) => handler.vt(Action::Print(cp)),
            crate::parser::Action::OscDispatch(command) => {
                osc_dispatch::dispatch(command, handler);
            }
            crate::parser::Action::DcsHook(input) => {
                handler.vt(Action::Payload(PayloadAction::DcsHook(input)));
            }
            crate::parser::Action::DcsPut(byte) => {
                handler.vt(Action::Payload(PayloadAction::DcsPut(byte)));
            }
            crate::parser::Action::DcsUnhook => {
                handler.vt(Action::Payload(PayloadAction::DcsUnhook));
            }
            crate::parser::Action::ApcStart => handler.vt(Action::Payload(PayloadAction::ApcStart)),
            crate::parser::Action::ApcPut(byte) => {
                handler.vt(Action::Payload(PayloadAction::ApcPut(byte)));
            }
            crate::parser::Action::ApcEnd => handler.vt(Action::Payload(PayloadAction::ApcEnd)),
        }
    }

    /// Decode one ground-state run, flushing carry only at an escape boundary.
    fn ground<'a>(&mut self, bytes: &'a [u8], handler: &mut impl Handler) -> &'a [u8] {
        let end = bytes
            .iter()
            .position(|&byte| byte == 0x1b)
            .unwrap_or(bytes.len());
        Self::text(&self.utf8.push(&bytes[..end]), handler);
        if end < bytes.len() {
            Self::text(&self.utf8.finish(), handler);
        }
        &bytes[end..]
    }

    /// Deliver a decoded run without interpreting the bytes a second time.
    fn text(text: &str, handler: &mut impl Handler) {
        for cp in text.chars() {
            if u32::from(cp) <= 0x0f {
                Self::execute(cp as u8, handler);
            } else {
                handler.vt(Action::Print(cp));
            }
        }
    }

    /// Dispatch an executed control at the parser's existing boundary.
    fn execute(byte: u8, handler: &mut impl Handler) {
        if byte > 0x7f {
            esc::dispatch(
                &crate::parser::Esc {
                    intermediates: &mut [],
                    r#final: byte - 0x40,
                },
                handler,
            );
            return;
        }
        let action = match byte {
            5 => Action::Query(QueryAction::Enquiry),
            7 => Action::Bell,
            8 => Action::Cursor(CursorAction::Backspace),
            9 => Action::Tab(TabAction::HorizontalTab(1)),
            10..=12 => Action::Cursor(CursorAction::Linefeed),
            13 => Action::Cursor(CursorAction::CarriageReturn),
            14 | 15 => Action::Charset(CharsetAction::InvokeCharset {
                bank: crate::CharsetActiveSlot::Gl,
                charset: if byte == 14 {
                    crate::CharsetSlot::G1
                } else {
                    crate::CharsetSlot::G0
                },
                locking: false,
            }),
            _ => return,
        };
        handler.vt(action);
    }

    /// Consume one byte, retaining the same state as slice delivery.
    pub fn next(&mut self, byte: u8, handler: &mut impl Handler) {
        self.next_slice(&[byte], handler);
    }
}
