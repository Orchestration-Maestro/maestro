//! Escape controls and character-set selection.

use super::{Action, Handler};
use crate::{
    Charset, CharsetActiveSlot as Bank, CharsetSlot as Slot, Mode, device_attributes::Req,
    parser::Esc,
};

/// Dispatch a parsed escape sequence without retaining borrowed intermediates.
pub(super) fn dispatch(input: &Esc<'_>, handler: &mut impl Handler) {
    let action = match (input.r#final, &*input.intermediates) {
        (b'B' | b'A' | b'0', [marker]) => {
            let slot = match marker {
                b'(' => Slot::G0,
                b')' => Slot::G1,
                b'*' => Slot::G2,
                b'+' => Slot::G3,
                _ => return,
            };
            let charset = match input.r#final {
                b'B' => Charset::Ascii,
                b'A' => Charset::British,
                _ => Charset::DecSpecial,
            };
            Action::ConfigureCharset { slot, charset }
        }
        (b'8', [b'#']) => Action::Decaln,
        (code, []) => match code {
            b'7' => Action::SaveCursor,
            b'8' => Action::RestoreCursor,
            b'D' => Action::Index,
            b'E' => Action::NextLine,
            b'H' => Action::TabSet,
            b'M' => Action::ReverseIndex,
            b'V' => Action::ProtectedModeIso,
            b'W' => Action::ProtectedModeOff,
            b'Z' => Action::DeviceAttributes(Req::Primary),
            b'c' => Action::FullReset,
            b'=' => Action::SetMode(Mode::KeypadKeys),
            b'>' => Action::ResetMode(Mode::KeypadKeys),
            _ => {
                shift(code, handler);
                return;
            }
        },
        _ => return,
    };
    handler.vt(action);
}

/// Select the requested bank and single-character or retained shift.
fn shift(code: u8, handler: &mut impl Handler) {
    let (bank, charset, locking) = match code {
        b'N' => (Bank::Gl, Slot::G2, true),
        b'O' => (Bank::Gl, Slot::G3, true),
        b'n' => (Bank::Gl, Slot::G2, false),
        b'o' => (Bank::Gl, Slot::G3, false),
        b'~' => (Bank::Gr, Slot::G1, false),
        b'}' => (Bank::Gr, Slot::G2, false),
        b'|' => (Bank::Gr, Slot::G3, false),
        _ => return,
    };
    handler.vt(Action::InvokeCharset {
        bank,
        charset,
        locking,
    });
}
