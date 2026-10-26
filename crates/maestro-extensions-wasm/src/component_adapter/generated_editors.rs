//! Editor requests through the generated host bindings.

use super::{Generated, route};
use crate::EditorOptions;
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{EditorAnswer, EditorRequest, EditorStyles, StyleRef};
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
