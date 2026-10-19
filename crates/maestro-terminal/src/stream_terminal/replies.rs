//! Query answers queued for the caller's PTY.

use std::io::Write;

use super::Handler;
use crate::{csi::SizeReportStyle, device_attributes, device_status::Request, stream::QueryAction};

impl Handler<'_> {
    /// Answer decoded queries without executing title-stack operations.
    pub(super) fn query(&mut self, action: QueryAction) {
        match action {
            QueryAction::DeviceAttributes(req) => self.device_attributes(req),
            QueryAction::DeviceStatus(req) => self.device_status(req),
            QueryAction::Xtversion => self
                .terminal
                .effects
                .pty_reply
                .extend_from_slice(b"\x1bP>|libghostty\x1b\\"),
            QueryAction::RequestMode(mode) => self.mode_report(mode.tag()),
            QueryAction::RequestModeUnknown(tag) => self.mode_report(tag),
            QueryAction::KittyKeyboardQuery => {
                let flags = self
                    .terminal
                    .screens
                    .active()
                    .kitty_keyboard
                    .current()
                    .int();
                let _ = write!(self.terminal.effects.pty_reply, "\x1b[?{flags}u");
            }
            QueryAction::SizeReport(SizeReportStyle::Csi21T) => {
                let title = self.terminal.get_title().unwrap_or_default().to_vec();
                let bytes = &mut self.terminal.effects.pty_reply;
                bytes.extend_from_slice(b"\x1b]l");
                bytes.extend_from_slice(&title);
                bytes.extend_from_slice(b"\x1b\\");
            }
            QueryAction::SizeReport(style) => self.size_report(style),
            QueryAction::Enquiry | QueryAction::TitlePush(_) | QueryAction::TitlePop(_) => {}
        }
    }

    /// Report raw caller geometry only when every dimension is known.
    fn size_report(&mut self, style: SizeReportStyle) {
        use crate::size_report::{self, Style};
        let size = self.terminal.size_report;
        if size.rows == 0 || size.columns == 0 || size.cell_width == 0 || size.cell_height == 0 {
            return;
        }
        let style = match style {
            SizeReportStyle::Csi14T => Style::Csi14T,
            SizeReportStyle::Csi16T => Style::Csi16T,
            SizeReportStyle::Csi18T => Style::Csi18T,
            SizeReportStyle::Csi21T => return,
        };
        let _ = size_report::encode(&mut self.terminal.effects.pty_reply, style, size);
    }

    /// Encode the default device attributes through their record owners.
    fn device_attributes(&mut self, req: device_attributes::Req) {
        let bytes = &mut self.terminal.effects.pty_reply;
        let _ = match req {
            device_attributes::Req::Primary => device_attributes::Primary::default().encode(bytes),
            device_attributes::Req::Secondary => {
                device_attributes::Secondary::default().encode(bytes)
            }
            device_attributes::Req::Tertiary => {
                device_attributes::Tertiary::default().encode(bytes)
            }
        };
    }

    /// Encode a status report, keeping origin-relative coordinates nonnegative.
    fn device_status(&mut self, req: Request) {
        let t = &mut *self.terminal;
        match req {
            Request::OperatingStatus => t.effects.pty_reply.extend_from_slice(b"\x1b[0n"),
            Request::CursorPosition => {
                let cursor = &t.screens.active().cursor;
                let (mut x, mut y) = (cursor.x, cursor.y);
                if t.modes.get(crate::Mode::Origin) {
                    x = x.saturating_sub(t.scrolling_region.left);
                    y = y.saturating_sub(t.scrolling_region.top);
                }
                let _ = write!(
                    t.effects.pty_reply,
                    "\x1b[{};{}R",
                    u32::from(y) + 1,
                    u32::from(x) + 1
                );
            }
            Request::ColorScheme => {
                if let Some(scheme) = t.color_scheme {
                    let _ = scheme.encode(&mut t.effects.pty_reply);
                }
            }
        }
    }

    /// Reuse the mode owner's namespace and state encoding.
    fn mode_report(&mut self, tag: crate::modes::ModeTag) {
        let report = self.terminal.modes.get_report(tag);
        let _ = report.encode(&mut self.terminal.effects.pty_reply);
    }
}
