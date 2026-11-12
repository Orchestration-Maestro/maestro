//! Theme operations read the original live publication on the local owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::ColorMode,
    local_resources::{Lease, held::Theme},
};
use maestro_extensions::{SourceInfo, Theme as NativeTheme, ThemeBg, ThemeColor};
use std::{cell::RefCell, rc::Rc, sync::Arc};

/// Store-safe identity retaining the original live theme until its final consumer.
#[derive(Clone)]
pub struct Endpoint(pub(crate) Arc<Lease>);

impl Endpoint {
    /// Runs one operation on the theme currently published, whose failure is the call's failure.
    fn with<T: Send + 'static>(
        &self,
        call: impl FnOnce(&Rc<NativeTheme>) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        self.0.run::<Theme, _>(move |live, _| {
            let theme = live.get().map_err(|error| error.to_string())?;
            call(&theme)
        })
    }

    /// Reads the currently published name.
    pub(crate) fn name(&self) -> Result<Option<String>, String> {
        self.with(|theme| Ok(theme.name().map(str::to_owned)))
    }

    /// Reads the authored path without normalization.
    pub(crate) fn source_path(&self) -> Result<Option<String>, String> {
        self.with(|theme| Ok(theme.source_path().map(str::to_owned)))
    }

    /// Reads current shared provenance as JSON.
    pub(crate) fn source_info(&self) -> Result<Option<String>, String> {
        self.with(|theme| {
            theme
                .source_info()
                .map(|source| serde_json::to_string(&*source.borrow()))
                .transpose()
                .map_err(|error| error.to_string())
        })
    }

    /// Replaces or clears provenance on the current prepared instance.
    pub(crate) fn set_source_info(&self, source: Option<String>) -> Result<(), String> {
        self.with(move |theme| {
            let source = source
                .as_deref()
                .map(crate::writer_reads::decode::<SourceInfo>)
                .transpose()?;
            theme.set_source_info(source.map(|source| Rc::new(RefCell::new(source))));
            Ok(())
        })
    }

    /// Reads the current conversion mode.
    pub(crate) fn get_color_mode(&self) -> Result<ColorMode, String> {
        self.with(|theme| Ok(theme.get_color_mode().into()))
    }

    /// Applies an authored foreground key.
    pub(crate) fn fg(&self, color: String, text: String) -> Result<String, String> {
        self.with(move |theme| {
            theme
                .fg(&ThemeColor::Named(color), &text)
                .map_err(|error| error.to_string())
        })
    }

    /// Applies an authored background key.
    pub(crate) fn bg(&self, color: String, text: String) -> Result<String, String> {
        self.with(move |theme| {
            theme
                .bg(&ThemeBg::Named(color), &text)
                .map_err(|error| error.to_string())
        })
    }

    /// Reads a foreground prefix.
    pub(crate) fn get_fg_ansi(&self, color: String) -> Result<String, String> {
        self.with(move |theme| {
            theme
                .get_fg_ansi(&ThemeColor::Named(color))
                .map(str::to_owned)
                .map_err(|error| error.to_string())
        })
    }

    /// Reads a background prefix.
    pub(crate) fn get_bg_ansi(&self, color: String) -> Result<String, String> {
        self.with(move |theme| {
            theme
                .get_bg_ansi(&ThemeBg::Named(color))
                .map(str::to_owned)
                .map_err(|error| error.to_string())
        })
    }

    /// Colors text with the border color of a thinking level.
    pub(crate) fn thinking_border(&self, level: String, text: String) -> Result<String, String> {
        self.with(move |theme| Ok(theme.get_thinking_border_color(&level)(&text)))
    }

    /// Colors text with the shell mode border color.
    pub(crate) fn bash_mode_border(&self, text: String) -> Result<String, String> {
        self.with(move |theme| Ok(theme.get_bash_mode_border_color()(&text)))
    }

    /// Decorates text with one native style, which needs no theme state beyond liveness.
    fn decorate(&self, style: fn(&str, bool) -> String, text: String) -> Result<String, String> {
        self.with(move |_| Ok(style(&text, true)))
    }

    /// Decorates text in bold.
    pub(crate) fn bold(&self, text: String) -> Result<String, String> {
        self.decorate(NativeTheme::bold, text)
    }

    /// Decorates text in italic.
    pub(crate) fn italic(&self, text: String) -> Result<String, String> {
        self.decorate(NativeTheme::italic, text)
    }

    /// Underlines text.
    pub(crate) fn underline(&self, text: String) -> Result<String, String> {
        self.decorate(NativeTheme::underline, text)
    }

    /// Inverts text colors.
    pub(crate) fn inverse(&self, text: String) -> Result<String, String> {
        self.decorate(NativeTheme::inverse, text)
    }

    /// Strikes through text.
    pub(crate) fn strikethrough(&self, text: String) -> Result<String, String> {
        self.decorate(NativeTheme::strikethrough, text)
    }
}
