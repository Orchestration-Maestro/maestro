//! Escape controls and character-set selection.

use crate::stream::{
    CharsetAction, CursorAction, EditAction, ModeAction, ProtectionAction, QueryAction, TabAction,
};

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
            Action::Charset(CharsetAction::ConfigureCharset { slot, charset })
        }
        (b'8', [b'#']) => Action::Edit(EditAction::Decaln),
        (code, []) => match code {
            b'7' => Action::Cursor(CursorAction::SaveCursor),
            b'8' => Action::Cursor(CursorAction::RestoreCursor),
            b'D' => Action::Cursor(CursorAction::Index),
            b'E' => Action::Cursor(CursorAction::NextLine),
            b'H' => Action::Tab(TabAction::TabSet),
            b'M' => Action::Cursor(CursorAction::ReverseIndex),
            b'V' => Action::Protection(ProtectionAction::ProtectedModeIso),
            b'W' => Action::Protection(ProtectionAction::ProtectedModeOff),
            b'Z' => Action::Query(QueryAction::DeviceAttributes(Req::Primary)),
            b'c' => Action::FullReset,
            b'=' => Action::Mode(ModeAction::SetMode(Mode::KeypadKeys)),
            b'>' => Action::Mode(ModeAction::ResetMode(Mode::KeypadKeys)),
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
    handler.vt(Action::Charset(CharsetAction::InvokeCharset {
        bank,
        charset,
        locking,
    }));
}
