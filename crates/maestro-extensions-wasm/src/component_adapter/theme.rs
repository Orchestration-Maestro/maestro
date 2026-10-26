//! Retained host theme behind the author's theme capability.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use super::Imports;
use crate::types::finite::encode_value;
use crate::{ColorMode, ExtensionResult, ModelThinkingLevel, SourceInfo, Theme, ThemePort};

/// Owned host theme kept by the facade.
pub(super) struct Retained<I: Imports> {
    /// The host the theme belongs to.
    imports: I,
    /// The retained host resource.
    pub(super) theme: I::Theme,
}

/// Wraps an independently owned host theme.
pub(super) fn theme<I: Imports>(imports: I, theme: I::Theme) -> Theme {
    Theme::supplied(Rc::new(Retained { imports, theme }))
}

impl<I: Imports> ThemePort for Retained<I> {
    fn name(&self) -> ExtensionResult<Option<String>> {
        self.imports.theme_name(&self.theme)
    }
    fn source_path(&self) -> ExtensionResult<Option<String>> {
        self.imports.theme_source_path(&self.theme)
    }
    fn source_info(&self) -> ExtensionResult<Option<SourceInfo>> {
        self.imports
            .theme_source_info(&self.theme)?
            .as_deref()
            .map(super::session::decode)
            .transpose()
    }
    fn set_source_info(&self, source: Option<SourceInfo>) -> ExtensionResult<()> {
        let source = source.as_ref().map(encode_value).transpose()?;
        self.imports
            .theme_set_source_info(&self.theme, source.as_deref())
    }
    fn fg(&self, color: &str, text: &str) -> ExtensionResult<String> {
        self.imports.theme_fg(&self.theme, color, text)
    }
    fn bg(&self, color: &str, text: &str) -> ExtensionResult<String> {
        self.imports.theme_bg(&self.theme, color, text)
    }
    fn bold(&self, text: &str) -> ExtensionResult<String> {
        self.imports.theme_bold(&self.theme, text)
    }
    fn italic(&self, text: &str) -> ExtensionResult<String> {
        self.imports.theme_italic(&self.theme, text)
    }
    fn underline(&self, text: &str) -> ExtensionResult<String> {
        self.imports.theme_underline(&self.theme, text)
    }
    fn inverse(&self, text: &str) -> ExtensionResult<String> {
        self.imports.theme_inverse(&self.theme, text)
    }
    fn strikethrough(&self, text: &str) -> ExtensionResult<String> {
        self.imports.theme_strikethrough(&self.theme, text)
    }
    fn get_fg_ansi(&self, color: &str) -> ExtensionResult<String> {
        self.imports.theme_get_fg_ansi(&self.theme, color)
    }
    fn get_bg_ansi(&self, color: &str) -> ExtensionResult<String> {
        self.imports.theme_get_bg_ansi(&self.theme, color)
    }
    fn get_color_mode(&self) -> ExtensionResult<ColorMode> {
        self.imports.theme_get_color_mode(&self.theme)
    }
    fn thinking_border(&self, level: ModelThinkingLevel, text: &str) -> ExtensionResult<String> {
        self.imports
            .theme_thinking_border(&self.theme, &level.to_string(), text)
    }
    fn bash_mode_border(&self, text: &str) -> ExtensionResult<String> {
        self.imports.theme_bash_mode_border(&self.theme, text)
    }
}
