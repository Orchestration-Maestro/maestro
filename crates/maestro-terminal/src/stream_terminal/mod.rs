//! Synchronous application of decoded operations to retained terminal state.

mod cursor;
mod modes;
mod write;
pub(crate) use write::WriteState;

use crate::{
    Terminal,
    stream::{
        Action, CharsetAction, CursorAction, EditAction, EraseAction, KeyboardAction, ModeAction,
        OscAction, PayloadAction, ProtectionAction, QueryAction, TabAction,
    },
};

/// Terminal consumer with retained cursor appearance defaults.
///
/// Applies screen state synchronously and consumes queries without replies.
/// Bells, external commands and application payloads have no external effects.
/// Each storage refusal ends only the current action; later input remains usable.
pub struct Handler<'a> {
    /// Terminal updated by each callback.
    pub terminal: &'a mut Terminal,
    /// Whether the cursor uses the configured default appearance.
    pub default_cursor: bool,
    /// Shape restored by a default cursor request or reset.
    pub default_cursor_style: crate::CursorStyle,
    /// Blinking restored by a default cursor request or reset.
    pub default_cursor_blink: bool,
}

impl<'a> Handler<'a> {
    /// Borrow a terminal with block, nonblinking cursor defaults.
    pub fn new(terminal: &'a mut Terminal) -> Self {
        Self {
            terminal,
            default_cursor: true,
            default_cursor_style: crate::CursorStyle::Block,
            default_cursor_blink: false,
        }
    }
}

impl crate::stream::Handler for Handler<'_> {
    fn vt(&mut self, action: Action<'_>) {
        match action {
            Action::Print(cp) => {
                let _ = self.terminal.print(cp);
            }
            Action::Cursor(action) => self.cursor(action),
            Action::Edit(action) => self.edit(action),
            Action::Erase(action) => self.erase(action),
            Action::Tab(action) => self.tab(action),
            Action::Mode(action) => self.mode(action),
            Action::Keyboard(action) => self.keyboard(action),
            Action::Charset(action) => self.charset(action),
            Action::Protection(action) => self.protection(action),
            Action::Osc(action) => self.osc(action),
            Action::Query(action) => Self::query(action),
            Action::Payload(action) => Self::payload(&action),
            Action::SetAttribute(attr) => {
                let _ = self.terminal.set_attribute(attr);
            }
            Action::ActiveStatusDisplay(display) => self.terminal.status_display = display,
            Action::MouseShiftCapture(value) => {
                self.terminal.flags.mouse_shift_capture = Some(value);
            }
            Action::CursorStyle(style) => self.cursor_style(style),
            Action::FullReset => {
                self.terminal.full_reset();
                self.cursor_style(crate::ansi::CursorStyle::Default);
            }
            Action::TopAndBottomMargin {
                top_left,
                bottom_right,
            } => self
                .terminal
                .set_top_and_bottom_margin(usize::from(top_left), usize::from(bottom_right)),
            Action::LeftAndRightMargin {
                top_left,
                bottom_right,
            } => self
                .terminal
                .set_left_and_right_margin(usize::from(top_left), usize::from(bottom_right)),
            Action::LeftAndRightMarginAmbiguous => self.margin_or_save(),
            // Readonly consumers have no bell effect.
            Action::Bell => {}
        }
    }
}

impl Handler<'_> {
    /// Apply cursor operations through their existing terminal owners.
    fn cursor(&mut self, action: CursorAction) {
        use CursorAction as A;
        let t = &mut *self.terminal;
        match action {
            A::Index => {
                let _ = t.index();
            }
            A::Linefeed => {
                let _ = t.linefeed();
            }
            A::NextLine => {
                if t.index().is_ok() {
                    t.carriage_return();
                }
            }
            A::ReverseIndex => {
                let _ = t.reverse_index();
            }
            A::Backspace => t.backspace(),
            A::CarriageReturn => t.carriage_return(),
            A::CursorUp(n) => t.cursor_up(usize::from(n)),
            A::CursorDown(n) => t.cursor_down(usize::from(n)),
            A::CursorLeft(n) => t.cursor_left(usize::from(n)),
            A::CursorRight(n) => t.cursor_right(usize::from(n)),
            A::CursorPos { row, col } => t.set_cursor_pos(usize::from(row), usize::from(col)),
            A::CursorCol(n) => self.axis(Some(usize::from(n)), None),
            A::CursorRow(n) => self.axis(None, Some(usize::from(n))),
            A::CursorColRelative(n) => self.relative_axis(n, false),
            A::CursorRowRelative(n) => self.relative_axis(n, true),
            A::SaveCursor => t.save_cursor(),
            A::RestoreCursor => t.restore_cursor(),
        }
    }

    /// Keep independent storage-failure boundaries for each edit.
    fn edit(&mut self, action: EditAction) {
        use EditAction as A;
        let t = &mut *self.terminal;
        match action {
            A::PrintRepeat(n) => {
                let _ = t.print_repeat(usize::from(n));
            }
            A::InsertBlanks(n) => t.insert_blanks(usize::from(n)),
            A::DeleteChars(n) => t.delete_chars(usize::from(n)),
            A::EraseChars(n) => t.erase_chars(usize::from(n)),
            A::InsertLines(n) => {
                let _ = t.insert_lines(usize::from(n));
            }
            A::DeleteLines(n) => {
                let _ = t.delete_lines(usize::from(n));
            }
            A::ScrollUp(n) => {
                let _ = t.scroll_up(usize::from(n));
            }
            A::ScrollDown(n) => {
                let _ = t.scroll_down(usize::from(n));
            }
            A::Decaln => {
                let _ = t.decaln();
            }
        }
    }

    /// Delegate erasure without duplicating protection or dirty-row policy.
    fn erase(&mut self, action: EraseAction) {
        use crate::csi::{EraseDisplay as D, EraseLine as L};
        use EraseAction as A;
        let t = &mut *self.terminal;
        match action {
            A::EraseDisplayBelow(p) => t.erase_display(D::Below, p),
            A::EraseDisplayAbove(p) => t.erase_display(D::Above, p),
            A::EraseDisplayComplete(p) => t.erase_display(D::Complete, p),
            A::EraseDisplayScrollback(p) => t.erase_display(D::Scrollback, p),
            A::EraseDisplayScrollComplete(p) => t.erase_display(D::ScrollComplete, p),
            A::EraseLineRight(p) => t.erase_line(L::Right, p),
            A::EraseLineLeft(p) => t.erase_line(L::Left, p),
            A::EraseLineComplete(p) => t.erase_line(L::Complete, p),
        }
    }

    /// Reuse existing tab destinations with a bounded repeated traversal.
    fn tab(&mut self, action: TabAction) {
        match action {
            TabAction::HorizontalTab(n) => self.tabs(n, false),
            TabAction::HorizontalTabBack(n) => self.tabs(n, true),
            TabAction::TabSet => self.terminal.tab_set(),
            TabAction::TabClearCurrent => self.terminal.tab_clear(crate::csi::TabClear::Current),
            TabAction::TabClearAll => self.terminal.tab_clear(crate::csi::TabClear::All),
            TabAction::TabReset => self.terminal.tab_reset(),
        }
    }

    /// Restore saved bits through the same side effects as explicit mode changes.
    fn mode(&mut self, action: ModeAction) {
        match action {
            ModeAction::SetMode(mode) => self.set_mode(mode, true),
            ModeAction::ResetMode(mode) => self.set_mode(mode, false),
            ModeAction::SaveMode(mode) => self.terminal.modes.save(mode),
            ModeAction::RestoreMode(mode) => {
                let enabled = self.terminal.modes.restore(mode);
                self.set_mode(mode, enabled);
            }
        }
    }

    /// Apply keyboard requests to the active screen's history.
    fn keyboard(&mut self, action: KeyboardAction) {
        use crate::kitty::key::SetMode;
        let t = &mut *self.terminal;
        match action {
            KeyboardAction::KittyKeyboardPush(flags) => {
                t.screens.active_mut().kitty_keyboard.push(flags);
            }
            KeyboardAction::KittyKeyboardPop(n) => {
                t.screens.active_mut().kitty_keyboard.pop(usize::from(n));
            }
            KeyboardAction::KittyKeyboardSet(flags) => t
                .screens
                .active_mut()
                .kitty_keyboard
                .set(SetMode::Set, flags),
            KeyboardAction::KittyKeyboardSetOr(flags) => t
                .screens
                .active_mut()
                .kitty_keyboard
                .set(SetMode::Or, flags),
            KeyboardAction::KittyKeyboardSetNot(flags) => t
                .screens
                .active_mut()
                .kitty_keyboard
                .set(SetMode::Not, flags),
            KeyboardAction::ModifyKeyFormat(format) => {
                t.flags.modify_other_keys_2 =
                    format == crate::ansi::ModifyKeyFormat::OtherKeysNumeric;
            }
        }
    }

    /// Apply character mapping using the existing charset owner.
    fn charset(&mut self, action: CharsetAction) {
        match action {
            CharsetAction::ConfigureCharset { slot, charset } => {
                self.terminal.configure_charset(slot, charset);
            }
            CharsetAction::InvokeCharset {
                bank,
                charset,
                locking,
            } => self.terminal.invoke_charset(bank, charset, locking),
        }
    }

    /// Select the existing character protection protocol.
    fn protection(&mut self, action: ProtectionAction) {
        use crate::ansi::ProtectedMode;
        let mode = match action {
            ProtectionAction::ProtectedModeOff => ProtectedMode::Off,
            ProtectionAction::ProtectedModeIso => ProtectedMode::Iso,
            ProtectionAction::ProtectedModeDec => ProtectedMode::Dec,
        };
        self.terminal.set_protected_mode(mode);
    }

    /// Apply retained screen commands; external effects stay outside this consumer.
    fn osc(&mut self, action: OscAction) {
        match action {
            OscAction::SemanticPrompt(cmd) => {
                let _ = self.terminal.semantic_prompt(&cmd);
            }
            OscAction::StartHyperlink { id, uri } => {
                let _ = self
                    .terminal
                    .screens
                    .active_mut()
                    .start_hyperlink(&uri, id.as_deref());
            }
            OscAction::EndHyperlink => self.terminal.screens.active_mut().end_hyperlink(),
            OscAction::WindowTitle(_)
            | OscAction::ClipboardContents { .. }
            | OscAction::ReportPwd(_)
            | OscAction::ColorOperation { .. }
            | OscAction::KittyColorReport(_) => {}
        }
    }

    /// Consume queries without producing external effects in the readonly handler.
    fn query(action: QueryAction) {
        match action {
            QueryAction::Enquiry
            | QueryAction::DeviceAttributes(_)
            | QueryAction::RequestMode(_)
            | QueryAction::RequestModeUnknown(_)
            | QueryAction::KittyKeyboardQuery
            | QueryAction::DeviceStatus(_)
            | QueryAction::Xtversion
            | QueryAction::SizeReport(_)
            | QueryAction::TitlePush(_)
            | QueryAction::TitlePop(_) => {}
        }
    }

    /// Consume DCS without terminal modification; APC execution is a separate consumer.
    fn payload(action: &PayloadAction<'_>) {
        match action {
            PayloadAction::DcsHook(_)
            | PayloadAction::DcsPut(_)
            | PayloadAction::DcsUnhook
            | PayloadAction::ApcStart
            | PayloadAction::ApcPut(_)
            | PayloadAction::ApcEnd => {}
        }
    }

    /// Resolve the parameterless horizontal-margin and cursor-save ambiguity.
    fn margin_or_save(&mut self) {
        if self
            .terminal
            .modes
            .get(crate::Mode::EnableLeftAndRightMargin)
        {
            self.terminal.set_left_and_right_margin(0, 0);
        } else {
            self.terminal.save_cursor();
        }
    }
}
