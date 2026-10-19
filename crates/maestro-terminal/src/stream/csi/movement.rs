//! Cursor, cell-edit and tabulation dispatch.

use super::super::{Action, Handler};
use crate::parser::Csi;

/// Dispatch commands with one count and no intermediate bytes.
pub(super) fn count(input: &Csi<'_>, handler: &mut impl Handler) {
    if !input.intermediates.is_empty() {
        return;
    }
    let value = match &*input.params {
        [] => 1,
        [value] => *value,
        _ => return,
    };
    let action = match input.r#final {
        b'A' | b'k' | b'F' => Action::CursorUp(value),
        b'B' | b'E' => Action::CursorDown(value),
        b'C' => Action::CursorRight(value),
        b'D' | b'j' => Action::CursorLeft(value),
        b'G' | b'`' => Action::CursorCol(value),
        b'I' => Action::HorizontalTab(value),
        b'L' => Action::InsertLines(value),
        b'M' => Action::DeleteLines(value),
        b'P' => Action::DeleteChars(value),
        b'S' => Action::ScrollUp(value),
        b'T' => Action::ScrollDown(value),
        b'X' => Action::EraseChars(value),
        b'Z' => Action::HorizontalTabBack(value),
        b'a' => Action::CursorColRelative(value.max(1)),
        b'b' => Action::PrintRepeat(value),
        b'd' => Action::CursorRow(value),
        b'e' => Action::CursorRowRelative(value.max(1)),
        b'@' => Action::InsertBlanks(value.max(1)),
        _ => return,
    };
    handler.vt(action);
    if matches!(input.r#final, b'E' | b'F') {
        handler.vt(Action::CarriageReturn);
    }
}

/// Retain both coordinates and their independent omitted defaults.
pub(super) fn position(input: &Csi<'_>, handler: &mut impl Handler) {
    if !input.intermediates.is_empty() {
        return;
    }
    let (row, col) = match &*input.params {
        [] => (1, 1),
        [row] => (*row, 1),
        [row, col] => (*row, *col),
        _ => return,
    };
    handler.vt(Action::CursorPos { row, col });
}

/// Dispatch tab controls with their distinct parameter and marker domains.
pub(super) fn tabs(input: &Csi<'_>, handler: &mut impl Handler) {
    let action = match (input.r#final, &*input.intermediates, &*input.params) {
        (b'W', [], [] | [0]) => Action::TabSet,
        (b'W', [], [2]) | (b'g', [], [] | [0]) => Action::TabClearCurrent,
        (b'W', [], [5]) | (b'g', [], [3]) => Action::TabClearAll,
        (b'W', [b'?'], [5]) => Action::TabReset,
        _ => return,
    };
    handler.vt(action);
}

/// Select only erase modes admitted by the byte protocol.
pub(super) fn erase(input: &Csi<'_>, handler: &mut impl Handler) {
    let protected = match &*input.intermediates {
        [] => false,
        [b'?'] => true,
        _ => return,
    };
    let mode = match &*input.params {
        [] => 0,
        [value] => *value,
        _ => return,
    };
    let action = match (input.r#final, mode) {
        (b'J', 0) => Action::EraseDisplayBelow(protected),
        (b'J', 1) => Action::EraseDisplayAbove(protected),
        (b'J', 2) => Action::EraseDisplayComplete(protected),
        (b'J', 3) => Action::EraseDisplayScrollback(protected),
        (b'J', 22) => Action::EraseDisplayScrollComplete(protected),
        (b'K', 0) => Action::EraseLineRight(protected),
        (b'K', 1) => Action::EraseLineLeft(protected),
        (b'K', 2) => Action::EraseLineComplete(protected),
        _ => return,
    };
    handler.vt(action);
}
