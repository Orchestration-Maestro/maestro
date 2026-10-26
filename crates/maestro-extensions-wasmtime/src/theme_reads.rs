//! Theme requests read the original live publication on the local owner.
use crate::{local_resources::Lease, theme_imports::ColorMode};
use maestro_extensions::{LiveTheme, SourceInfo, Theme, ThemeBg, ThemeColor};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};

/// Catchable completion of one native theme operation.
type Reply<T> = Sender<Result<T, String>>;

/// Exact canonical theme operands and owned replies.
pub(crate) enum Request {
    /// Reads the currently published name.
    Name(Reply<Option<String>>),
    /// Reads the authored path without normalization.
    SourcePath(Reply<Option<String>>),
    /// Reads current shared provenance as JSON.
    SourceInfo(Reply<Option<String>>),
    /// Replaces or clears provenance on the current prepared instance.
    SetSourceInfo(Option<String>, Reply<()>),
    /// Applies an authored foreground key.
    Fg(String, String, Reply<String>),
    /// Applies an authored background key.
    Bg(String, String, Reply<String>),
    /// Decorates text in bold.
    Bold(String, Reply<String>),
    /// Decorates text in italic.
    Italic(String, Reply<String>),
    /// Underlines text.
    Underline(String, Reply<String>),
    /// Inverts text colors.
    Inverse(String, Reply<String>),
    /// Strikes through text.
    Strikethrough(String, Reply<String>),
    /// Reads a foreground prefix.
    FgAnsi(String, Reply<String>),
    /// Reads a background prefix.
    BgAnsi(String, Reply<String>),
    /// Reads the current conversion mode.
    ColorMode(Reply<ColorMode>),
    /// Colors text with the border color of a thinking level.
    ThinkingBorder(String, String, Reply<String>),
    /// Colors text with the shell mode border color.
    BashModeBorder(String, Reply<String>),
}

impl Request {
    /// Returns a catchable failure after the last native owner has ended.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Name(reply) | Self::SourcePath(reply) | Self::SourceInfo(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::SetSourceInfo(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::ColorMode(reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
            Self::Fg(_, _, reply)
            | Self::Bg(_, _, reply)
            | Self::ThinkingBorder(_, _, reply)
            | Self::BashModeBorder(_, reply)
            | Self::Bold(_, reply)
            | Self::Italic(_, reply)
            | Self::Underline(_, reply)
            | Self::Inverse(_, reply)
            | Self::Strikethrough(_, reply)
            | Self::FgAnsi(_, reply)
            | Self::BgAnsi(_, reply) => {
                let _ = reply.send(Err(error.to_owned()));
            }
        }
    }

    /// Delegates to the current prepared instance without retaining a registry borrow.
    pub(crate) fn answer(self, live: &LiveTheme) {
        let theme = match live.get() {
            Ok(theme) => theme,
            Err(error) => return self.reject(&error.to_string()),
        };
        match self {
            Self::Name(reply) => {
                let _ = reply.send(Ok(theme.name().map(str::to_owned)));
            }
            Self::SourcePath(reply) => {
                let _ = reply.send(Ok(theme.source_path().map(str::to_owned)));
            }
            Self::SourceInfo(reply) => {
                let result = theme
                    .source_info()
                    .map(|source| serde_json::to_string(&*source.borrow()))
                    .transpose();
                let _ = reply.send(result.map_err(|error| error.to_string()));
            }
            Self::SetSourceInfo(source, reply) => {
                let result = source
                    .as_deref()
                    .map(crate::writer_reads::decode::<SourceInfo>)
                    .transpose();
                let _ = reply.send(result.map(|source| {
                    theme.set_source_info(source.map(|source| Rc::new(RefCell::new(source))));
                }));
            }
            Self::ColorMode(reply) => {
                let _ = reply.send(Ok(theme.get_color_mode().into()));
            }
            request => request.text(&theme),
        }
    }

    /// Applies native color validation and text decorations without a parallel formatter.
    fn text(self, theme: &Rc<Theme>) {
        let (result, reply) = match self {
            Self::Fg(color, text, reply) => (
                theme
                    .fg(&ThemeColor::Named(color), &text)
                    .map_err(|error| error.to_string()),
                reply,
            ),
            Self::Bg(color, text, reply) => (
                theme
                    .bg(&ThemeBg::Named(color), &text)
                    .map_err(|error| error.to_string()),
                reply,
            ),
            Self::FgAnsi(color, reply) => (
                theme
                    .get_fg_ansi(&ThemeColor::Named(color))
                    .map(str::to_owned)
                    .map_err(|error| error.to_string()),
                reply,
            ),
            Self::BgAnsi(color, reply) => (
                theme
                    .get_bg_ansi(&ThemeBg::Named(color))
                    .map(str::to_owned)
                    .map_err(|error| error.to_string()),
                reply,
            ),
            Self::ThinkingBorder(level, text, reply) => {
                (Ok(theme.get_thinking_border_color(&level)(&text)), reply)
            }
            Self::BashModeBorder(text, reply) => {
                (Ok(theme.get_bash_mode_border_color()(&text)), reply)
            }
            Self::Bold(text, reply) => (Ok(Theme::bold(&text, true)), reply),
            Self::Italic(text, reply) => (Ok(Theme::italic(&text, true)), reply),
            Self::Underline(text, reply) => (Ok(Theme::underline(&text, true)), reply),
            Self::Inverse(text, reply) => (Ok(Theme::inverse(&text, true)), reply),
            Self::Strikethrough(text, reply) => (Ok(Theme::strikethrough(&text, true)), reply),
            request => return request.reject("expected theme text operation"),
        };
        let _ = reply.send(result);
    }
}

/// Store-safe identity retaining the original live theme until its final consumer.
#[derive(Clone)]
pub(crate) struct Endpoint(pub(crate) Arc<Lease>);
