//! Typed control-sequence dispatch from retained parser views.

mod keyboard;
mod modes;
mod movement;
mod window;

use super::Handler;

/// Apply one parsed control sequence synchronously.
pub(super) fn dispatch(input: &crate::parser::Csi<'_>, handler: &mut impl Handler) {
    match input.r#final {
        b'r' | b's' if input.intermediates.is_empty() => window::margins(input, handler),
        b'h' | b'l' | b'r' | b's' => modes::dispatch(input, handler),
        b'p' | b'q' => modes::request(input, handler),
        b'm' if input.intermediates.is_empty() => {
            for attr in (crate::sgr::Parser {
                params: input.params,
                params_sep: input.params_sep,
                idx: 0,
            }) {
                handler.vt(super::Action::SetAttribute(attr));
            }
        }
        b'm' | b'n' => modes::modifier(input, handler),
        b'u' => keyboard::dispatch(input, handler),
        b'c' | b't' | b'}' => window::dispatch(input, handler),
        b'H' | b'f' => movement::position(input, handler),
        b'W' | b'g' => movement::tabs(input, handler),
        b'J' | b'K' => movement::erase(input, handler),
        _ => movement::count(input, handler),
    }
}
