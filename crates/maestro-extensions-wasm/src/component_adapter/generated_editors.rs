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
