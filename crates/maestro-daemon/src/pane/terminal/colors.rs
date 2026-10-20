//! Cached host theme and child override policy.

use super::{PaneCore, PaneTerminal};
use crate::pane::osc::{DefaultColorEvent, DefaultColorQuery};
use crate::terminal_theme::{DefaultColorKind, HostAppearance, TerminalTheme};
use maestro_process::pty::foreground::ForegroundJob;
use maestro_terminal::{Terminal, color::Rgb, screen_set::Key};

impl PaneTerminal {
    /// Refresh host colours without replacing child-owned dynamic or palette entries.
    pub fn apply_host_terminal_theme(&self, theme: TerminalTheme) {
        let Ok(mut core) = self.core.lock() else {
            return;
        };
        core.host_terminal_theme = theme;
        let foreground = !core.child_default_foreground_changed;
        let background = !core.child_default_background_changed;
        if foreground && background {
            core.transient_default_color_owner_pgid = None;
        }
        let mut palette = maestro_terminal::color::DEFAULT;
        for (index, color) in theme.palette.iter().enumerate() {
            if let Some(color) = color {
                palette[index] = rgb(*color);
            }
        }
        core.terminal.set_default_palette(&palette);
        for (enabled, kind) in [
            (foreground, DefaultColorKind::Foreground),
            (background, DefaultColorKind::Background),
        ] {
            if enabled {
                write_color(&mut core.terminal, &theme, kind);
            }
        }
    }

    /// Update colour-scheme reports through the engine's existing transition policy.
    pub fn apply_host_terminal_appearance(
        &self,
        appearance: Option<HostAppearance>,
    ) -> Option<Vec<u8>> {
        use maestro_terminal::device_status::ColorScheme;
        let mut core = self.core.lock().ok()?;
        let pending = core.terminal.take_pty_reply();
        if !pending.is_empty() {
            core.pending_pty_responses.push(pending);
        }
        core.terminal
            .set_color_scheme(appearance.map(|appearance| match appearance {
                HostAppearance::Dark => ColorScheme::Dark,
                HostAppearance::Light => ColorScheme::Light,
            }));
        let bytes = core.terminal.take_pty_reply();
        (!bytes.is_empty()).then_some(bytes)
    }

    /// Whether a foreground job owns a transient dynamic-colour override.
    #[must_use]
    pub fn has_transient_default_color_override(&self) -> bool {
        self.core
            .lock()
            .is_ok_and(|core| core.transient_default_color_owner_pgid.is_some())
    }

    /// Whether obtaining fresh foreground process facts could restore the host theme.
    #[must_use]
    pub fn should_probe_host_terminal_theme_restore(&self) -> bool {
        self.core.lock().is_ok_and(|core| {
            core.transient_default_color_owner_pgid.is_some()
                && !core.host_terminal_theme.is_empty()
                && core.terminal.active_screen() == Key::Primary
        })
    }

    /// Restore cached host defaults after the owning job has returned to the shell.
    /// Foreground observations are supplied by the native process owner.
    pub fn maybe_restore_host_terminal_theme(
        &self,
        shell_pid: u32,
        foreground_job: Option<&ForegroundJob>,
    ) -> bool {
        let Ok(mut core) = self.core.lock() else {
            return false;
        };
        let Some(owner) = core.transient_default_color_owner_pgid else {
            return false;
        };
        if core.host_terminal_theme.is_empty()
            || !super::super::osc::should_restore_host_terminal_theme(
                owner,
                shell_pid,
                core.terminal.active_screen() == Key::Alternate,
                foreground_job,
            )
        {
            return false;
        }
        core.transient_default_color_owner_pgid = None;
        core.child_default_foreground_changed = false;
        core.child_default_background_changed = false;
        let theme = core.host_terminal_theme;
        write_color(&mut core.terminal, &theme, DefaultColorKind::Foreground);
        write_color(&mut core.terminal, &theme, DefaultColorKind::Background);
        tracing::info!(
            owner_pgid = owner,
            "restored host terminal default colors after transient override"
        );
        true
    }
}

/// Write one dynamic default using the existing host-theme encoder.
fn write_color(terminal: &mut Terminal, theme: &TerminalTheme, kind: DefaultColorKind) {
    let color = match kind {
        DefaultColorKind::Foreground => theme.foreground,
        DefaultColorKind::Background => theme.background,
    };
    if let Some(color) = color {
        terminal
            .write(crate::terminal_theme::osc_set_default_color_sequence(kind, color).as_bytes());
    } else {
        terminal.write(crate::terminal_theme::osc_reset_default_color_sequence(kind).as_bytes());
    }
}

/// Apply ownership and answer the pane's host-aware query policy.
pub(super) fn respond(core: &mut PaneCore, event: DefaultColorEvent) -> Option<Vec<u8>> {
    match event {
        DefaultColorEvent::Set(query) => {
            mark_child_color(core, query, true);
            None
        }
        DefaultColorEvent::Reset(query) => {
            mark_child_color(core, query, false);
            let kind = match query {
                DefaultColorQuery::Foreground => Some(DefaultColorKind::Foreground),
                DefaultColorQuery::Background => Some(DefaultColorKind::Background),
                DefaultColorQuery::Cursor => None,
            };
            if let Some(kind) = kind {
                write_color(&mut core.terminal, &core.host_terminal_theme, kind);
            }
            None
        }
        DefaultColorEvent::Query(query) => {
            let color = match query {
                DefaultColorQuery::Foreground if !core.child_default_foreground_changed => {
                    core.host_terminal_theme.foreground.map(rgb)
                }
                DefaultColorQuery::Background if !core.child_default_background_changed => {
                    core.host_terminal_theme.background.map(rgb)
                }
                DefaultColorQuery::Cursor => core.terminal.effective_cursor_color().or_else(|| {
                    if core.child_default_foreground_changed {
                        core.terminal.effective_foreground_color()
                    } else {
                        core.host_terminal_theme
                            .foreground
                            .map(rgb)
                            .or_else(|| core.terminal.effective_foreground_color())
                    }
                }),
                _ => None,
            }?;
            Some(rgb_response(&query.osc_number().to_string(), color))
        }
        DefaultColorEvent::PaletteQuery(index) => {
            let color = core.terminal.colors.palette.current[usize::from(index)];
            Some(rgb_response(&format!("4;{index}"), color))
        }
    }
}

/// Track only the two dynamic colours governed by host refreshes.
fn mark_child_color(core: &mut PaneCore, query: DefaultColorQuery, changed: bool) {
    match query {
        DefaultColorQuery::Foreground => core.child_default_foreground_changed = changed,
        DefaultColorQuery::Background => core.child_default_background_changed = changed,
        DefaultColorQuery::Cursor => {}
    }
}

/// Represent host channels with the engine's colour record.
fn rgb(color: crate::terminal_theme::RgbColor) -> Rgb {
    Rgb {
        r: color.r,
        g: color.g,
        b: color.b,
    }
}

/// Encode the pane's canonical ST-terminated sixteen-bit colour reply.
fn rgb_response(command: &str, color: Rgb) -> Vec<u8> {
    let mut response = format!("\x1b]{command};").into_bytes();
    let _ = color.encode_rgb16(&mut response);
    response.extend_from_slice(b"\x1b\\");
    response
}

/// Replace only the final matching colour reply, preserving earlier engine reports.
pub(super) fn replace_reply(bytes: &mut Vec<u8>, event: DefaultColorEvent) {
    let prefix = match event {
        DefaultColorEvent::Query(query) => format!("\x1b]{};rgb:", query.osc_number()),
        DefaultColorEvent::PaletteQuery(index) => format!("\x1b]4;{index};rgb:"),
        DefaultColorEvent::Set(_) | DefaultColorEvent::Reset(_) => return,
    };
    let Some(start) = bytes
        .windows(prefix.len())
        .rposition(|window| window == prefix.as_bytes())
    else {
        return;
    };
    let Some(end) = bytes[start..]
        .iter()
        .position(|byte| *byte == 7)
        .map(|offset| start + offset + 1)
        .or_else(|| {
            bytes[start..]
                .windows(2)
                .position(|window| window == b"\x1b\\")
                .map(|offset| start + offset + 2)
        })
    else {
        return;
    };
    bytes.drain(start..end);
}
