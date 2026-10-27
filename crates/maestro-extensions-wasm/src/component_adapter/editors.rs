//! Host editors and custom editors behind the author's editor facades.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::keys::HostKeybindings;
use super::lists::{WidgetHandler, set_handler, stage_handler};
use super::widget_imports::{
    EditorAnswer, EditorRequest, EditorStyles, HandlerSlot, WidgetImports,
};
use super::widgets::{Builder, HostWidget, stage_style, style_ref};
use crate::{
    CustomEditorPort, EditorOptions, EditorPort, EditorShortcut, EditorTheme, ExtensionResult,
    KeybindingsManager, Style, TUI, TextHandler, WidgetNotice,
};

/// Runs `request` with the editor styles of `theme`, staging its authored styles and keeping
/// them only when the host accepts.
pub(super) fn with_editor_styles<I: WidgetImports, T>(
    imports: &I,
    theme: &EditorTheme,
    request: impl FnOnce(EditorStyles<'_, I>) -> ExtensionResult<T>,
) -> ExtensionResult<T> {
    let list = &theme.select_list;
    let styles: Vec<_> = [
        &theme.border_color,
        &list.selected_prefix,
        &list.selected_text,
        &list.description,
        &list.scroll_info,
        &list.no_match,
    ]
    .into_iter()
    .map(|style| stage_style(imports, style))
    .collect();
    let mut refs = styles.iter().map(style_ref);
    let border = refs.next().ok_or("an editor needs a border style")?;
    let result = request(EditorStyles {
        border,
        select_list: refs.collect(),
    });
    let accepted = result.is_ok();
    for staged in styles {
        staged.settle(accepted);
    }
    result
}

impl<I: WidgetImports> Builder<I> {
    /// Builds an editor, or a custom editor with `keybindings`, staging its authored styles.
    pub(super) fn build_editor(
        &self,
        tui: &TUI,
        theme: &EditorTheme,
        keybindings: Option<&KeybindingsManager>,
        options: EditorOptions,
    ) -> ExtensionResult<I::Widget> {
        let imports = &self.0;
        let tui = tui
            .host::<super::factories::Terminal<I>>()
            .ok_or("an editor needs a terminal the host lent")?;
        let manager = keybindings
            .map(|keybindings| {
                keybindings
                    .host::<HostKeybindings<I>>()
                    .ok_or("a custom editor needs a keybindings manager the host keeps")
            })
            .transpose()?;
        with_editor_styles(imports, theme, |theme| match manager {
            None => imports.new_editor(&tui.tui, theme, options),
            Some(manager) => imports.new_custom_editor(&tui.tui, theme, &manager.manager, options),
        })
    }
}

impl<I: WidgetImports> HostWidget<I> {
    /// Sends one editor request.
    fn editor(&self, request: EditorRequest<'_, I>) -> ExtensionResult<EditorAnswer> {
        self.imports.widget_editor(&self.widget, request)
    }
    /// Sends one editor request that answers nothing.
    fn change(&self, request: EditorRequest<'_, I>) -> ExtensionResult<()> {
        self.editor(request).map(drop)
    }
    /// Sends one editor request that answers text.
    fn text(&self, request: EditorRequest<'_, I>) -> ExtensionResult<String> {
        match self.editor(request)? {
            EditorAnswer::Text(text) => Ok(text),
            _ => Err("the host answered no text".to_owned()),
        }
    }
    /// Sends one editor request that answers a number.
    fn number(&self, request: EditorRequest<'_, I>) -> ExtensionResult<f64> {
        match self.editor(request)? {
            EditorAnswer::Number(number) => Ok(number),
            _ => Err("the host answered no number".to_owned()),
        }
    }
    /// Sends one editor request that answers a flag.
    fn flag(&self, request: EditorRequest<'_, I>) -> ExtensionResult<bool> {
        match self.editor(request)? {
            EditorAnswer::Flag(flag) => Ok(flag),
            _ => Err("the host answered no flag".to_owned()),
        }
    }
    /// Replaces or removes one handler.
    fn handler(&self, slot: HandlerSlot, handler: Option<WidgetHandler>) -> ExtensionResult<()> {
        set_handler(&self.imports, &self.widget, slot, handler)
    }
}

impl<I: super::providers::EditorImports> EditorPort for HostWidget<I> {
    fn get_text(&self) -> ExtensionResult<String> {
        self.text(EditorRequest::GetText)
    }
    fn set_text(&self, text: &str) -> ExtensionResult<()> {
        self.change(EditorRequest::SetText(text))
    }
    fn get_expanded_text(&self) -> ExtensionResult<String> {
        self.text(EditorRequest::GetExpandedText)
    }
    fn get_lines(&self) -> ExtensionResult<Vec<String>> {
        match self.editor(EditorRequest::GetLines)? {
            EditorAnswer::Lines(lines) => Ok(lines),
            _ => Err("the host answered no lines".to_owned()),
        }
    }
    fn get_cursor(&self) -> ExtensionResult<(u32, u32)> {
        match self.editor(EditorRequest::GetCursor)? {
            EditorAnswer::Cursor(line, col) => Ok((line, col)),
            _ => Err("the host answered no cursor".to_owned()),
        }
    }
    fn insert_text_at_cursor(&self, text: &str) -> ExtensionResult<()> {
        self.change(EditorRequest::InsertTextAtCursor(text))
    }
    fn add_to_history(&self, text: &str) -> ExtensionResult<()> {
        self.change(EditorRequest::AddToHistory(text))
    }
    fn get_padding_x(&self) -> ExtensionResult<f64> {
        self.number(EditorRequest::GetPaddingX)
    }
    fn set_padding_x(&self, padding: f64) -> ExtensionResult<()> {
        self.change(EditorRequest::SetPaddingX(padding))
    }
    fn get_autocomplete_max_visible(&self) -> ExtensionResult<f64> {
        self.number(EditorRequest::GetAutocompleteMaxVisible)
    }
    fn set_autocomplete_max_visible(&self, max_visible: f64) -> ExtensionResult<()> {
        self.change(EditorRequest::SetAutocompleteMaxVisible(max_visible))
    }
    fn disable_submit(&self) -> ExtensionResult<bool> {
        self.flag(EditorRequest::GetDisableSubmit)
    }
    fn set_disable_submit(&self, disabled: bool) -> ExtensionResult<()> {
        self.change(EditorRequest::SetDisableSubmit(disabled))
    }
    fn is_showing_autocomplete(&self) -> ExtensionResult<bool> {
        self.flag(EditorRequest::IsShowingAutocomplete)
    }
    fn focused(&self) -> ExtensionResult<bool> {
        self.flag(EditorRequest::GetFocused)
    }
    fn set_focused(&self, focused: bool) -> ExtensionResult<()> {
        self.change(EditorRequest::SetFocused(focused))
    }
    fn border_color(&self) -> ExtensionResult<Style> {
        let style = self.imports.widget_border_color(&self.widget)?;
        Ok(super::widgets::host_style(&self.imports, style))
    }
    fn set_border_color(&self, color: &Style) -> ExtensionResult<()> {
        let staged = stage_style(&self.imports, color);
        let result = self.change(EditorRequest::SetBorderColor(style_ref(&staged)));
        staged.settle(result.is_ok());
        result
    }
    fn set_on_submit(&self, handler: Option<TextHandler>) -> ExtensionResult<()> {
        self.handler(HandlerSlot::Submit, handler.map(WidgetHandler::Text))
    }
    fn on_change(&self) -> ExtensionResult<Option<TextHandler>> {
        Ok(self.change.borrow().clone())
    }
    fn set_on_change(&self, handler: Option<TextHandler>) -> ExtensionResult<()> {
        self.handler(
            HandlerSlot::Change,
            handler.clone().map(WidgetHandler::Text),
        )?;
        drop(self.change.replace(handler));
        Ok(())
    }
    fn handle_input(&self, data: &str) -> ExtensionResult<()> {
        self.imports.widget_handle_input(&self.widget, data)
    }
    fn set_autocomplete_provider(
        &self,
        provider: std::rc::Rc<dyn crate::AutocompleteProvider>,
    ) -> ExtensionResult<()> {
        let identity = super::providers::stage(&self.imports, provider);
        let result = self
            .imports
            .widget_set_autocomplete_provider(&self.widget, &identity.handle);
        identity.settle(result.is_ok());
        result
    }
}

impl<I: WidgetImports> CustomEditorPort for HostWidget<I> {
    fn on_action(&self, action: &str, handler: WidgetNotice) -> ExtensionResult<()> {
        let identity = stage_handler(&self.imports, WidgetHandler::Notice(handler));
        let result = self
            .imports
            .widget_on_action(&self.widget, action, &identity.handle);
        identity.settle(result.is_ok());
        result
    }
    fn set_on_escape(&self, handler: Option<WidgetNotice>) -> ExtensionResult<()> {
        self.handler(HandlerSlot::Escape, handler.map(WidgetHandler::Notice))
    }
    fn set_on_ctrl_d(&self, handler: Option<WidgetNotice>) -> ExtensionResult<()> {
        self.handler(HandlerSlot::CtrlD, handler.map(WidgetHandler::Notice))
    }
    fn set_on_paste_image(&self, handler: Option<WidgetNotice>) -> ExtensionResult<()> {
        self.handler(HandlerSlot::PasteImage, handler.map(WidgetHandler::Notice))
    }
    fn set_on_extension_shortcut(&self, handler: Option<EditorShortcut>) -> ExtensionResult<()> {
        let handler = handler.map(WidgetHandler::Shortcut);
        self.handler(HandlerSlot::ExtensionShortcut, handler)
    }
}
