//! Editors built with an editor factory another extension set, reached through the host.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::any::Any;
use std::rc::Rc;

use super::keys::HostKeybindings;
use super::lists::{WidgetHandler, stage_handler};
use super::providers::{EditorImports, stage};
use super::widget_imports::StyleRef;
use super::widgets::{Builder, HostWidget, stage_style, style_ref};
use crate::{
    AutocompleteProvider, CallbackEmitter, ComponentView, CustomEditor, CustomEditorComponent,
    EditorComponent, EditorFactory, EditorFactoryOutcome, EditorTheme, ExtensionResult,
    HostWidgetHandle, KeybindingsManager, Style, TUI, TextHandler,
};

/// One editor operation on an editor another extension's factory built.
pub(crate) enum ForeignRequest<'a, I: EditorImports> {
    /// The text.
    GetText,
    /// Replaces the text.
    SetText(&'a str),
    /// Replaces or removes the submission handler staged under this identity.
    SetOnSubmit(Option<&'a I::Callback>),
    /// The change handler.
    OnChange,
    /// Replaces or removes the change handler staged under this identity.
    SetOnChange(Option<&'a I::Callback>),
    /// Adds text to the history.
    AddToHistory(&'a str),
    /// Inserts text at the cursor.
    InsertTextAtCursor(&'a str),
    /// The text with markers expanded.
    GetExpandedText,
    /// The border style.
    BorderColor,
    /// Replaces the border style.
    SetBorderColor(StyleRef<'a, I>),
    /// Replaces the side padding.
    SetPaddingX(f64),
    /// Replaces the completion row count.
    SetAutocompleteMaxVisible(f64),
    /// Replaces the completion provider with the provider staged under this identity.
    SetAutocompleteProvider(&'a I::Callback),
}

/// What an editor another extension's factory built answered.
pub(crate) enum ForeignAnswer<I: EditorImports> {
    /// The operation ran.
    Done,
    /// The editor lacks the operation.
    Absent,
    /// Text.
    Text(String),
    /// The change handler the host lent, when one is installed.
    Handler(Option<I::TextCallback>),
    /// The border style the host lent.
    Style(I::Style),
}

/// An editor another extension's factory built; `C` is the custom editor it extends, if any.
pub(crate) struct ForeignEditor<I: EditorImports, C = ()> {
    /// The host widget.
    widget: Rc<HostWidget<I>>,
    /// The custom editor facade over the same widget.
    custom: C,
}

impl<I: EditorImports, C> ForeignEditor<I, C> {
    /// Sends one request.
    fn request(&self, request: ForeignRequest<'_, I>) -> ExtensionResult<ForeignAnswer<I>> {
        self.widget
            .imports
            .foreign_editor(&self.widget.widget, request)
    }
    /// Sends a request that answers nothing.
    fn change(&self, request: ForeignRequest<'_, I>) -> ExtensionResult<()> {
        match self.request(request)? {
            ForeignAnswer::Done => Ok(()),
            _ => Err("the host answered no completion".to_owned()),
        }
    }
    /// Sends a request of an optional operation.
    fn optional(&self, request: ForeignRequest<'_, I>) -> ExtensionResult<Option<()>> {
        match self.request(request)? {
            ForeignAnswer::Done => Ok(Some(())),
            ForeignAnswer::Absent => Ok(None),
            _ => Err("the host answered no completion".to_owned()),
        }
    }
    /// Replaces or removes the submission or change handler.
    fn handler(&self, handler: Option<TextHandler>, submit: bool) -> ExtensionResult<()> {
        let staged = handler
            .map(|handler| stage_handler(&self.widget.imports, WidgetHandler::Text(handler)));
        let identity = staged.as_ref().map(|identity| &identity.handle);
        let request = if submit {
            ForeignRequest::SetOnSubmit(identity)
        } else {
            ForeignRequest::SetOnChange(identity)
        };
        let result = self.change(request);
        if let Some(identity) = staged {
            identity.settle(result.is_ok());
        }
        result
    }
}

impl<I: EditorImports, C> ComponentView for ForeignEditor<I, C> {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.widget
            .imports
            .widget_render(&self.widget.widget, width)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.widget
            .imports
            .widget_handle_input(&self.widget.widget, data)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.widget.imports.widget_invalidate(&self.widget.widget)
    }
}

impl<I: EditorImports, C> EditorComponent for ForeignEditor<I, C> {
    fn get_text(&self, _: CallbackEmitter<'_>) -> ExtensionResult<String> {
        match self.request(ForeignRequest::GetText)? {
            ForeignAnswer::Text(text) => Ok(text),
            _ => Err("the host answered no text".to_owned()),
        }
    }
    fn set_text(&self, text: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.change(ForeignRequest::SetText(text))
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::handle_input(self, data, emitter)
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        self.handler(handler, true)
    }
    fn on_change(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        match self.request(ForeignRequest::OnChange)? {
            ForeignAnswer::Handler(handler) => Ok(handler
                .map(|handler| super::editor_factories::lent_text(&self.widget.imports, handler))),
            _ => Err("the host answered no handler".to_owned()),
        }
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        self.handler(handler, false)
    }
    fn add_to_history(&self, text: &str, _: CallbackEmitter<'_>) -> ExtensionResult<Option<()>> {
        self.optional(ForeignRequest::AddToHistory(text))
    }
    fn insert_text_at_cursor(
        &self,
        text: &str,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        self.optional(ForeignRequest::InsertTextAtCursor(text))
    }
    fn get_expanded_text(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<String>> {
        match self.request(ForeignRequest::GetExpandedText)? {
            ForeignAnswer::Text(text) => Ok(Some(text)),
            ForeignAnswer::Absent => Ok(None),
            _ => Err("the host answered no text".to_owned()),
        }
    }
    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider>,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        let identity = stage(&self.widget.imports, provider);
        let result = self.optional(ForeignRequest::SetAutocompleteProvider(&identity.handle));
        identity.settle(result.is_ok());
        result
    }
    fn border_color(&self, _: CallbackEmitter<'_>) -> ExtensionResult<Option<Style>> {
        match self.request(ForeignRequest::BorderColor)? {
            ForeignAnswer::Style(style) => Ok(Some(super::widgets::host_style(
                &self.widget.imports,
                style,
            ))),
            ForeignAnswer::Absent => Ok(None),
            _ => Err("the host answered no style".to_owned()),
        }
    }
    fn set_border_color(
        &self,
        color: Style,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        let staged = stage_style(&self.widget.imports, &color);
        let result = self.optional(ForeignRequest::SetBorderColor(style_ref(&staged)));
        staged.settle(result.is_ok());
        result
    }
    fn set_padding_x(&self, padding: f64, _: CallbackEmitter<'_>) -> ExtensionResult<Option<()>> {
        self.optional(ForeignRequest::SetPaddingX(padding))
    }
    fn set_autocomplete_max_visible(
        &self,
        max_visible: f64,
        _: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        self.optional(ForeignRequest::SetAutocompleteMaxVisible(max_visible))
    }
}

impl<I: EditorImports> CustomEditorComponent for ForeignEditor<I, CustomEditor> {
    fn custom_editor(&self) -> &CustomEditor {
        &self.custom
    }
}

/// An editor factory another extension set, kept as the host resource that builds with it.
pub(super) fn factory(factory: Rc<dyn Any>) -> EditorFactory {
    Rc::new(move |tui, theme, keybindings, _| {
        super::widget_host()?.foreign_editor(&*factory, &tui, &theme, &keybindings)
    })
}

impl<I: EditorImports> Builder<I> {
    /// Builds an editor with another extension's factory, staging authored styles.
    pub(super) fn build_foreign(
        &self,
        factory: &dyn Any,
        tui: &TUI,
        theme: &EditorTheme,
        keybindings: &KeybindingsManager,
    ) -> ExtensionResult<EditorFactoryOutcome> {
        let imports = &self.0;
        let factory = factory
            .downcast_ref::<I::EditorFactory>()
            .ok_or("the editor factory belongs to another host")?;
        let tui = tui
            .host::<super::factories::Terminal<I>>()
            .ok_or("an editor needs a terminal the host lent")?;
        let keybindings = keybindings
            .host::<HostKeybindings<I>>()
            .ok_or("an editor needs a keybindings manager the host keeps")?;
        let (widget, custom) = super::editors::with_editor_styles(imports, theme, |theme| {
            imports.editor_factory_build(factory, &tui.tui, theme, &keybindings.manager)
        })?;
        let widget = HostWidget::shared(imports, widget);
        if !custom {
            return Ok(EditorFactoryOutcome::Editor(Rc::new(ForeignEditor {
                widget,
                custom: (),
            })));
        }
        let parts = (
            HostWidgetHandle::new(Rc::clone(&widget)),
            Rc::clone(&widget) as Rc<dyn crate::EditorPort>,
            Rc::clone(&widget) as Rc<dyn crate::CustomEditorPort>,
        );
        Ok(EditorFactoryOutcome::CustomEditor(Rc::new(ForeignEditor {
            widget,
            custom: CustomEditor::hosted(parts),
        })))
    }
}
