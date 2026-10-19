//! Keyboard protocol requests using checked flags and full-width pop counts.

use super::super::{Action, Handler};
use crate::{kitty::key::Flags, parser::Csi};

/// Retain each keyboard branch's distinct arity and default rules.
pub(super) fn dispatch(input: &Csi<'_>, handler: &mut impl Handler) {
    let params = &*input.params;
    let action = match &*input.intermediates {
        [] => Action::RestoreCursor,
        [b'?'] => Action::KittyKeyboardQuery,
        [b'>'] => {
            let value = if let [value] = params { *value } else { 0 };
            let Some(flags) = Flags::from_int(value) else {
                return;
            };
            Action::KittyKeyboardPush(flags)
        }
        [b'<'] => Action::KittyKeyboardPop(if let [value] = params { *value } else { 1 }),
        [b'='] => {
            let Some(flags) = Flags::from_int(params.first().copied().unwrap_or(0)) else {
                return;
            };
            match params.get(1).copied().unwrap_or(1) {
                1 => Action::KittyKeyboardSet(flags),
                2 => Action::KittyKeyboardSetOr(flags),
                3 => Action::KittyKeyboardSetNot(flags),
                _ => return,
            }
        }
        _ => return,
    };
    handler.vt(action);
}
