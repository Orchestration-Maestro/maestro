//! Styled emission shared by the content walk and keyboard-state formatters.

use super::{Format, PageFormatter, ScreenFormatter, TerminalFormatter};
use crate::{Style, page::Cell};
use std::fmt::{self, Write};

/// Optional screen keyboard state.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScreenExtra {
    /// Emit retained enhanced keyboard flags and nested pushes when enabled.
    pub kitty_keyboard: bool,
}

/// Optional terminal keyboard state.
#[derive(Clone, Copy, Debug, Default)]
pub struct TerminalExtra {
    /// Emit extended modifier-key mode when enabled.
    pub keyboard: bool,
    /// Screen keyboard state emitted before terminal keyboard state.
    pub screen: ScreenExtra,
}

impl TerminalFormatter<'_> {
    /// Emit terminal keyboard mode only for styled output.
    pub(super) fn format_keyboard(&self, writer: &mut impl Write) -> fmt::Result {
        if self.opts.emit == Format::Vt
            && self.extra.keyboard
            && self.terminal.flags.modify_other_keys_2
        {
            writer.write_str("\x1b[>4;2m")?;
        }
        Ok(())
    }
}

impl ScreenFormatter<'_> {
    /// Emit nondefault screen keyboard flags only for styled output.
    pub(super) fn format_keyboard(&self, writer: &mut impl Write) -> fmt::Result {
        if self.opts.emit == Format::Vt && self.extra.kitty_keyboard {
            self.screen.kitty_keyboard.replay_ansi(writer)?;
        }
        Ok(())
    }
}

impl PageFormatter<'_> {
    /// Emit configured default colours before page content.
    pub(super) fn format_defaults(&self, writer: &mut impl Write) -> fmt::Result {
        if self.opts.emit != Format::Vt {
            return Ok(());
        }
        for (code, rgb) in [(10, self.opts.foreground), (11, self.opts.background)] {
            if let Some(rgb) = rgb {
                write!(
                    writer,
                    "\x1b]{code};rgb:{:02x}/{:02x}/{:02x}\x1b\\",
                    rgb.r, rgb.g, rgb.b
                )?;
            }
        }
        Ok(())
    }

    /// Change styles only when the cell's resolved style differs.
    pub(super) fn format_style(
        &self,
        writer: &mut impl Write,
        cell: Cell,
        style: &mut Style,
    ) -> fmt::Result {
        if self.opts.emit != Format::Vt {
            return Ok(());
        }
        let next = self.page.cell_style(cell);
        if next != *style {
            next.format_vt(writer, self.opts.palette)?;
            *style = next;
        }
        Ok(())
    }
}
