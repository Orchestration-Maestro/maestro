//! Mode lists resolved in their original protocol namespace and order.

use crate::stream::{KeyboardAction, ModeAction, ProtectionAction, QueryAction};

use super::super::{Action, Handler};

/// Resolve supported mode numbers without removing repeated entries.
pub(super) fn dispatch(input: &crate::parser::Csi<'_>, handler: &mut impl Handler) {
    if input.r#final == b's' && input.intermediates == *b">" {
        match &*input.params {
            [] | [0] => handler.vt(Action::MouseShiftCapture(false)),
            [1] => handler.vt(Action::MouseShiftCapture(true)),
            _ => (),
        }
        return;
    }
    let ansi = match &*input.intermediates {
        [] if matches!(input.r#final, b'h' | b'l') => true,
        [b'?'] => false,
        _ => return,
    };
    for &number in &*input.params {
        let Some(mode) = crate::Mode::from_number(number, ansi) else {
            continue;
        };
        let action = match input.r#final {
            b'h' => Action::Mode(ModeAction::SetMode(mode)),
            b'l' => Action::Mode(ModeAction::ResetMode(mode)),
            b's' => Action::Mode(ModeAction::SaveMode(mode)),
            b'r' => Action::Mode(ModeAction::RestoreMode(mode)),
            _ => return,
        };
        handler.vt(action);
    }
}

/// Decode status requests and cursor/protection selections.
pub(super) fn request(input: &crate::parser::Csi<'_>, handler: &mut impl Handler) {
    use crate::ansi::CursorStyle as C;
    let action = match (input.r#final, &*input.intermediates, &*input.params) {
        (b'p', [b'$'], [value]) => mode_request(*value, true),
        (b'p', [b'?', b'$'], [value]) => mode_request(*value, false),
        (b'q', [b'>'], _) => Action::Query(QueryAction::Xtversion),
        (b'q', [b' '], [] | [0]) => Action::CursorStyle(C::Default),
        (b'q', [b' '], [1]) => Action::CursorStyle(C::BlinkingBlock),
        (b'q', [b' '], [2]) => Action::CursorStyle(C::SteadyBlock),
        (b'q', [b' '], [3]) => Action::CursorStyle(C::BlinkingUnderline),
        (b'q', [b' '], [4]) => Action::CursorStyle(C::SteadyUnderline),
        (b'q', [b' '], [5]) => Action::CursorStyle(C::BlinkingBar),
        (b'q', [b' '], [6]) => Action::CursorStyle(C::SteadyBar),
        (b'q', [b'"'], [] | [0 | 2]) => Action::Protection(ProtectionAction::ProtectedModeOff),
        (b'q', [b'"'], [1]) => Action::Protection(ProtectionAction::ProtectedModeDec),
        _ => return,
    };
    handler.vt(action);
}

/// Preserve unknown mode numbers with the original namespace.
fn mode_request(value: u16, ansi: bool) -> Action<'static> {
    crate::Mode::from_number(value, ansi).map_or(
        Action::Query(QueryAction::RequestModeUnknown(crate::modes::ModeTag {
            value,
            ansi,
        })),
        |value| Action::Query(QueryAction::RequestMode(value)),
    )
}

/// Dispatch modifier encoding and device status without changing operand policy.
pub(super) fn modifier(input: &crate::parser::Csi<'_>, handler: &mut impl Handler) {
    use crate::ansi::ModifyKeyFormat as F;
    let action = match (input.r#final, &*input.intermediates, &*input.params) {
        (b'm', [b'>'], []) => Action::Keyboard(KeyboardAction::ModifyKeyFormat(F::Legacy)),
        (b'm', [b'>'], [selector] | [selector, _]) => {
            let format = match selector {
                0 => F::Legacy,
                1 => F::CursorKeys,
                2 => F::FunctionKeys,
                4 if input.params.get(1) == Some(&2) => F::OtherKeysNumeric,
                4 => F::OtherKeysNone,
                _ => return,
            };
            Action::Keyboard(KeyboardAction::ModifyKeyFormat(format))
        }
        (b'n', [b'>'], _) => {
            Action::Keyboard(KeyboardAction::ModifyKeyFormat(F::OtherKeysNumericExcept))
        }
        (b'n', [], [value]) => {
            let Some(req) = crate::device_status::req_from_int(*value, false) else {
                return;
            };
            Action::Query(QueryAction::DeviceStatus(req))
        }
        (b'n', [b'?'], [value]) => {
            let Some(req) = crate::device_status::req_from_int(*value, true) else {
                return;
            };
            Action::Query(QueryAction::DeviceStatus(req))
        }
        _ => return,
    };
    handler.vt(action);
}
