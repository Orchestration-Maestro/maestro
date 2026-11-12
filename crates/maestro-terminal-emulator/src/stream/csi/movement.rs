//! Cursor, cell-edit and tabulation dispatch.

use crate::stream::{CursorAction, EditAction, EraseAction, TabAction};

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
        b'A' | b'k' | b'F' => Action::Cursor(CursorAction::CursorUp(value)),
        b'B' | b'E' => Action::Cursor(CursorAction::CursorDown(value)),
        b'C' => Action::Cursor(CursorAction::CursorRight(value)),
        b'D' | b'j' => Action::Cursor(CursorAction::CursorLeft(value)),
        b'G' | b'`' => Action::Cursor(CursorAction::CursorCol(value)),
        b'I' => Action::Tab(TabAction::HorizontalTab(value)),
        b'L' => Action::Edit(EditAction::InsertLines(value)),
        b'M' => Action::Edit(EditAction::DeleteLines(value)),
        b'P' => Action::Edit(EditAction::DeleteChars(value)),
        b'S' => Action::Edit(EditAction::ScrollUp(value)),
        b'T' => Action::Edit(EditAction::ScrollDown(value)),
        b'X' => Action::Edit(EditAction::EraseChars(value)),
        b'Z' => Action::Tab(TabAction::HorizontalTabBack(value)),
        b'a' => Action::Cursor(CursorAction::CursorColRelative(value.max(1))),
        b'b' => Action::Edit(EditAction::PrintRepeat(value)),
        b'd' => Action::Cursor(CursorAction::CursorRow(value)),
        b'e' => Action::Cursor(CursorAction::CursorRowRelative(value.max(1))),
        b'@' => Action::Edit(EditAction::InsertBlanks(value.max(1))),
        _ => return,
    };
    handler.vt(action);
    if matches!(input.r#final, b'E' | b'F') {
        handler.vt(Action::Cursor(CursorAction::CarriageReturn));
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
    handler.vt(Action::Cursor(CursorAction::CursorPos { row, col }));
}

/// Dispatch tab controls with their distinct parameter and marker domains.
pub(super) fn tabs(input: &Csi<'_>, handler: &mut impl Handler) {
    let action = match (input.r#final, &*input.intermediates, &*input.params) {
        (b'W', [], [] | [0]) => Action::Tab(TabAction::TabSet),
        (b'W', [], [2]) | (b'g', [], [] | [0]) => Action::Tab(TabAction::TabClearCurrent),
        (b'W', [], [5]) | (b'g', [], [3]) => Action::Tab(TabAction::TabClearAll),
        (b'W', [b'?'], [5]) => Action::Tab(TabAction::TabReset),
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
        (b'J', 0) => Action::Erase(EraseAction::EraseDisplayBelow(protected)),
        (b'J', 1) => Action::Erase(EraseAction::EraseDisplayAbove(protected)),
        (b'J', 2) => Action::Erase(EraseAction::EraseDisplayComplete(protected)),
        (b'J', 3) => Action::Erase(EraseAction::EraseDisplayScrollback(protected)),
        (b'J', 22) => Action::Erase(EraseAction::EraseDisplayScrollComplete(protected)),
        (b'K', 0) => Action::Erase(EraseAction::EraseLineRight(protected)),
        (b'K', 1) => Action::Erase(EraseAction::EraseLineLeft(protected)),
        (b'K', 2) => Action::Erase(EraseAction::EraseLineComplete(protected)),
        _ => return,
    };
    handler.vt(action);
}
