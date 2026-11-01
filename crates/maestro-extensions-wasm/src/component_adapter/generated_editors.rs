//! Editor requests through the generated host bindings.

use super::{Generated, route};
use crate::EditorOptions;
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{EditorAnswer, EditorRequest, EditorStyles, StyleRef};
use crate::component_adapter::{Exports, callbacks, lists::WidgetHandler};
use crate::types::ExtensionResult;

/// The wire form of selection list styles in field order.
pub(super) fn select_styles<'a>(
    styles: &[StyleRef<'a, Generated>],
) -> ExtensionResult<host::SelectListStyles<'a>> {
    let mut styles = styles.iter().map(super::widgets::style);
    let mut next = || {
        styles
            .next()
            .ok_or_else(|| "a selection list style is missing".to_owned())
    };
    Ok(host::SelectListStyles {
        selected_prefix: next()?,
        selected_text: next()?,
        description: next()?,
        scroll_info: next()?,
        no_match: next()?,
    })
}

/// The wire form of editor styles and options.
pub(super) fn wire<'a>(
    theme: &EditorStyles<'a, Generated>,
    options: EditorOptions,
) -> ExtensionResult<(host::EditorStyles<'a>, host::EditorOptions)> {
    let styles = host::EditorStyles {
        border_color: super::widgets::style(&theme.border),
        select_list: select_styles(&theme.select_list)?,
    };
    let options = host::EditorOptions {
        padding_x: options.padding_x,
        autocomplete_max_visible: options.autocomplete_max_visible,
    };
    Ok((styles, options))
}

/// Sends one editor request, driving the route of a text change.
pub(super) fn request(
    widget: &host::Widget,
    request: EditorRequest<'_, Generated>,
) -> ExtensionResult<EditorAnswer> {
    use host::EditorRequest as Wire;
    let wire = match request {
        EditorRequest::SetText(text) => {
            return route::drive(&widget.set_editor_text(text)).map(|()| EditorAnswer::Done);
        }
        EditorRequest::InsertTextAtCursor(text) => {
            return route::drive(&widget.insert_editor_text(text)).map(|()| EditorAnswer::Done);
        }
        EditorRequest::GetText => Wire::GetText,
        EditorRequest::GetExpandedText => Wire::GetExpandedText,
        EditorRequest::GetLines => Wire::GetLines,
        EditorRequest::GetCursor => Wire::GetCursor,
        EditorRequest::AddToHistory(text) => Wire::AddToHistory(text.to_owned()),
        EditorRequest::GetPaddingX => Wire::GetPaddingX,
        EditorRequest::SetPaddingX(padding) => Wire::SetPaddingX(padding),
        EditorRequest::GetAutocompleteMaxVisible => Wire::GetAutocompleteMaxVisible,
        EditorRequest::SetAutocompleteMaxVisible(max) => Wire::SetAutocompleteMaxVisible(max),
        EditorRequest::GetDisableSubmit => Wire::GetDisableSubmit,
        EditorRequest::SetDisableSubmit(disabled) => Wire::SetDisableSubmit(disabled),
        EditorRequest::IsShowingAutocomplete => Wire::IsShowingAutocomplete,
        EditorRequest::GetFocused => Wire::GetFocused,
        EditorRequest::SetFocused(focused) => Wire::SetFocused(focused),
        EditorRequest::SetBorderColor(color) => Wire::SetBorderColor(super::widgets::style(&color)),
    };
    Ok(match widget.editor(&wire)? {
        host::EditorAnswer::Done => EditorAnswer::Done,
        host::EditorAnswer::Text(text) => EditorAnswer::Text(text),
        host::EditorAnswer::Lines(lines) => EditorAnswer::Lines(lines),
        host::EditorAnswer::Cursor((line, col)) => EditorAnswer::Cursor(line, col),
        host::EditorAnswer::Number(number) => EditorAnswer::Number(number),
        host::EditorAnswer::Flag(flag) => EditorAnswer::Flag(flag),
    })
}

/// Runs an editor factory with its lent terminal, styles and keybindings, and transfers what
/// it built.
pub(super) fn editor_factory(
    handler: u32,
    tui: host::Tui,
    theme: host::EditorTheme,
    keybindings: host::KeybindingsManager,
    emitter: &host::CallbackEmitter,
) -> ExtensionResult<host::EditorBuilt> {
    let list = theme.select_list;
    let styles = vec![
        list.selected_prefix,
        list.selected_text,
        list.description,
        list.scroll_info,
        list.no_match,
    ];
    let lent = crate::component_adapter::LentEditor {
        tui,
        theme: (theme.border_color, styles),
        keybindings,
    };
    let outcome = Exports::new(Generated).invoke_editor_factory(handler, lent, emitter)?;
    crate::component_adapter::editor_factories::transfer(&Generated, &outcome)
}

/// Runs one host request on an authored editor and transfers what it answered.
pub(super) fn authored(
    handler: u32,
    request: host::AuthoredEditorRequest,
    emitter: &host::CallbackEmitter,
) -> ExtensionResult<host::AuthoredEditorAnswer> {
    use crate::component_adapter::{AuthoredAnswer, AuthoredRequest};
    use host::AuthoredEditorRequest as Wire;
    let request = match request {
        Wire::GetText => AuthoredRequest::GetText,
        Wire::SetText(text) => AuthoredRequest::SetText(text),
        Wire::SetOnSubmit(callback) => AuthoredRequest::SetOnSubmit(callback),
        Wire::OnChange => AuthoredRequest::OnChange,
        Wire::SetOnChange(callback) => AuthoredRequest::SetOnChange(callback),
        Wire::AddToHistory(text) => AuthoredRequest::AddToHistory(text),
        Wire::InsertTextAtCursor(text) => AuthoredRequest::InsertTextAtCursor(text),
        Wire::GetExpandedText => AuthoredRequest::GetExpandedText,
        Wire::BorderColor => AuthoredRequest::BorderColor,
        Wire::SetBorderColor(style) => AuthoredRequest::SetBorderColor(style),
        Wire::SetPaddingX(padding) => AuthoredRequest::SetPaddingX(padding),
        Wire::SetAutocompleteMaxVisible(max) => AuthoredRequest::SetAutocompleteMaxVisible(max),
        Wire::SetAutocompleteProvider(provider) => {
            AuthoredRequest::SetAutocompleteProvider(provider)
        }
    };
    let identity = || callbacks::Identity::<Generated>::new(&Generated);
    Ok(
        match Exports::new(Generated).invoke_editor(handler, request, emitter)? {
            AuthoredAnswer::Done => host::AuthoredEditorAnswer::Done,
            AuthoredAnswer::Absent => host::AuthoredEditorAnswer::Absent,
            AuthoredAnswer::Text(text) => host::AuthoredEditorAnswer::Text(text),
            AuthoredAnswer::Handler(handler) => {
                host::AuthoredEditorAnswer::Handler(handler.map(|handler| {
                    identity().transfer(std::rc::Rc::new(WidgetHandler::Text(handler)))
                }))
            }
            AuthoredAnswer::Style(style) => {
                host::AuthoredEditorAnswer::Style(identity().transfer(style))
            }
        },
    )
}

/// Runs one host request on an authored completion provider.
pub(super) async fn provider(
    handler: u32,
    request: host::ProviderRequest,
    signal: Option<host::AbortSignal>,
) -> ExtensionResult<host::ProviderAnswer> {
    use crate::component_adapter::{ProviderAnswer, ProviderRequest};
    let at = |(line, col): (u32, u32)| crate::CursorPosition { line, col };
    let request = match request {
        host::ProviderRequest::Suggestions((lines, cursor, force)) => {
            ProviderRequest::Suggestions(lines, at(cursor), force)
        }
        host::ProviderRequest::Apply((lines, cursor, item, prefix)) => {
            ProviderRequest::Apply(lines, at(cursor), item, prefix)
        }
        host::ProviderRequest::Trigger((lines, cursor)) => {
            ProviderRequest::Trigger(lines, at(cursor))
        }
    };
    let answer = Exports::new(Generated)
        .invoke_provider(handler, request, signal)
        .await?;
    Ok(match answer {
        ProviderAnswer::Suggestions(suggestions) => {
            host::ProviderAnswer::Suggestions(suggestions.map(|suggestions| {
                host::AutocompleteSuggestions {
                    items: suggestions.items,
                    prefix: suggestions.prefix,
                }
            }))
        }
        ProviderAnswer::Applied(result) => host::ProviderAnswer::Applied(host::CompletionResult {
            lines: result.lines,
            cursor_line: result.cursor_line,
            cursor_col: result.cursor_col,
        }),
        ProviderAnswer::Trigger(decision) => host::ProviderAnswer::Trigger(decision),
    })
}

/// Runs a provider factory on the lent current provider and transfers what it returned.
pub(super) fn provider_factory(
    handler: u32,
    current: host::CompletionProvider,
    emitter: &host::CallbackEmitter,
) -> ExtensionResult<host::Callback> {
    let provider = Exports::new(Generated).invoke_provider_factory(handler, current, emitter)?;
    Ok(callbacks::Identity::<Generated>::new(&Generated).transfer(provider))
}

/// Requests suggestions from a lent provider with a host signal.
pub(super) async fn suggestions(
    provider: &host::CompletionProvider,
    (lines, cursor, force): (&[String], crate::CursorPosition, Option<bool>),
    signal: &host::AbortSignal,
) -> ExtensionResult<Option<crate::AutocompleteSuggestions>> {
    let at = (cursor.line, cursor.col);
    let suggestions = provider
        .get_suggestions(lines.to_vec(), at, signal, force)
        .await?;
    Ok(
        suggestions.map(|suggestions| crate::AutocompleteSuggestions {
            items: suggestions.items,
            prefix: suggestions.prefix,
        }),
    )
}

/// Applies a completion through a lent provider, running the nested calls it makes first.
pub(super) fn apply(
    provider: &host::CompletionProvider,
    (lines, cursor): (&[String], crate::CursorPosition),
    item: &crate::AutocompleteItem,
    prefix: &str,
) -> ExtensionResult<crate::CompletionResult> {
    let route = provider.apply_completion(lines, (cursor.line, cursor.col), item, prefix);
    match route::value(&route)? {
        host::RouteValue::Completion(result) => Ok(crate::CompletionResult {
            lines: result.lines,
            cursor_line: result.cursor_line,
            cursor_col: result.cursor_col,
        }),
        _ => Err("extension route result kind mismatch".into()),
    }
}

/// Reads a lent provider's file-completion decision, running the nested calls it makes first.
pub(super) fn trigger(
    provider: &host::CompletionProvider,
    (lines, cursor): (&[String], crate::CursorPosition),
) -> ExtensionResult<Option<bool>> {
    let route = provider.should_trigger_file_completion(lines, (cursor.line, cursor.col));
    match route::value(&route)? {
        host::RouteValue::Trigger(decision) => Ok(decision),
        _ => Err("extension route result kind mismatch".into()),
    }
}

impl crate::component_adapter::EditorImports for Generated {
    type EditorFactory = host::EditorFactory;
    fn editor_factory_build(
        &self,
        factory: &host::EditorFactory,
        tui: &host::Tui,
        theme: EditorStyles<'_, Generated>,
        keybindings: &host::KeybindingsManager,
    ) -> ExtensionResult<(host::Widget, bool)> {
        let (styles, _) = wire(&theme, EditorOptions::default())?;
        match route::value(&factory.build(tui, &styles, keybindings))? {
            host::RouteValue::Editor(built) => Ok((built.widget, built.custom)),
            _ => Err("extension route result kind mismatch".into()),
        }
    }
    fn foreign_editor(
        &self,
        widget: &host::Widget,
        request: crate::component_adapter::ForeignRequest<'_, Generated>,
    ) -> ExtensionResult<crate::component_adapter::ForeignAnswer<Generated>> {
        foreign(widget, request)
    }
    fn widget_border_color(&self, widget: &host::Widget) -> ExtensionResult<host::Style> {
        widget.border_color()
    }
    type TextCallback = host::TextCallback;
    fn text_callback(&self, callback: &host::TextCallback, text: &str) -> ExtensionResult<()> {
        route::drive(&callback.call(text))
    }
    fn widget_set_autocomplete_provider(
        &self,
        widget: &host::Widget,
        provider: &host::Callback,
    ) -> ExtensionResult<()> {
        widget.set_autocomplete_provider(provider)
    }
    type Provider = host::CompletionProvider;
    fn provider_suggestions<'a>(
        &'a self,
        provider: &'a host::CompletionProvider,
        request: (&'a [String], crate::CursorPosition, Option<bool>),
        signal: &'a host::AbortSignal,
    ) -> crate::ExtensionFuture<'a, Option<crate::AutocompleteSuggestions>> {
        Box::pin(suggestions(provider, request, signal))
    }
    fn provider_apply(
        &self,
        provider: &host::CompletionProvider,
        at: (&[String], crate::CursorPosition),
        item: &crate::AutocompleteItem,
        prefix: &str,
    ) -> ExtensionResult<crate::CompletionResult> {
        apply(provider, at, item, prefix)
    }
    fn provider_trigger(
        &self,
        provider: &host::CompletionProvider,
        at: (&[String], crate::CursorPosition),
    ) -> ExtensionResult<Option<bool>> {
        trigger(provider, at)
    }
}

/// One editor operation on an editor another extension's factory built.
fn foreign(
    widget: &host::Widget,
    request: crate::component_adapter::ForeignRequest<'_, Generated>,
) -> ExtensionResult<crate::component_adapter::ForeignAnswer<Generated>> {
    use crate::component_adapter::{ForeignAnswer, ForeignRequest};
    use host::ForeignEditorRequest as Wire;
    let wire = match request {
        ForeignRequest::GetText => Wire::GetText,
        ForeignRequest::SetText(text) => Wire::SetText(text.to_owned()),
        ForeignRequest::SetOnSubmit(handler) => Wire::SetOnSubmit(handler),
        ForeignRequest::OnChange => Wire::OnChange,
        ForeignRequest::SetOnChange(handler) => Wire::SetOnChange(handler),
        ForeignRequest::AddToHistory(text) => Wire::AddToHistory(text.to_owned()),
        ForeignRequest::InsertTextAtCursor(text) => Wire::InsertTextAtCursor(text.to_owned()),
        ForeignRequest::GetExpandedText => Wire::GetExpandedText,
        ForeignRequest::BorderColor => Wire::BorderColor,
        ForeignRequest::SetBorderColor(style) => {
            Wire::SetBorderColor(super::widgets::style(&style))
        }
        ForeignRequest::SetPaddingX(padding) => Wire::SetPaddingX(padding),
        ForeignRequest::SetAutocompleteMaxVisible(max) => Wire::SetAutocompleteMaxVisible(max),
        ForeignRequest::SetAutocompleteProvider(provider) => {
            Wire::SetAutocompleteProvider(provider)
        }
    };
    let host::RouteValue::EditorAnswer(answer) = route::value(&widget.foreign_editor(&wire))?
    else {
        return Err("extension route result kind mismatch".into());
    };
    Ok(match answer {
        host::ForeignEditorAnswer::Done => ForeignAnswer::Done,
        host::ForeignEditorAnswer::Absent => ForeignAnswer::Absent,
        host::ForeignEditorAnswer::Text(text) => ForeignAnswer::Text(text),
        host::ForeignEditorAnswer::Handler(handler) => ForeignAnswer::Handler(handler),
        host::ForeignEditorAnswer::Style(style) => ForeignAnswer::Style(style),
    })
}
