//! The component host's editors: the shared fake, with the wire forms of their requests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use wasmtime::component::Resource;

use super::host::{Route, State};
use super::widget_component::WidgetResource;
use super::widget_editors::{self, Answer, Editor, Request};
use super::widget_host::Build;
use crate::bindings::host_side::maestro::extension::host;
use crate::component_adapter::HandlerSlot;

/// The adapter slot a wire slot names.
pub fn slot(slot: host::HandlerSlot) -> HandlerSlot {
    match slot {
        host::HandlerSlot::Select => HandlerSlot::Select,
        host::HandlerSlot::Cancel => HandlerSlot::Cancel,
        host::HandlerSlot::SelectionChange => HandlerSlot::SelectionChange,
        host::HandlerSlot::Abort => HandlerSlot::Abort,
        host::HandlerSlot::Submit => HandlerSlot::Submit,
        host::HandlerSlot::Change => HandlerSlot::Change,
        host::HandlerSlot::Escape => HandlerSlot::Escape,
        host::HandlerSlot::CtrlD => HandlerSlot::CtrlD,
        host::HandlerSlot::PasteImage => HandlerSlot::PasteImage,
        host::HandlerSlot::ExtensionShortcut => HandlerSlot::ExtensionShortcut,
    }
}

/// The wire form of an editor answer.
pub fn wire_answer(answer: Answer) -> host::EditorAnswer {
    match answer {
        Answer::Done => host::EditorAnswer::Done,
        Answer::Text(text) => host::EditorAnswer::Text(text),
        Answer::Lines(lines) => host::EditorAnswer::Lines(lines),
        Answer::Cursor(line, col) => host::EditorAnswer::Cursor((line, col)),
        Answer::Number(number) => host::EditorAnswer::Number(number),
        Answer::Flag(flag) => host::EditorAnswer::Flag(flag),
    }
}

impl State {
    /// Records an editor widget, keeps its state and lends it.
    pub(super) fn build_editor(
        &mut self,
        theme: &host::EditorStyles,
        keys: Option<u32>,
        options: host::EditorOptions,
    ) -> wasmtime::Result<Result<Resource<WidgetResource>, String>> {
        let border = self.paint(&theme.border_color)?;
        let list = &theme.select_list;
        let list = [
            &list.selected_prefix,
            &list.selected_text,
            &list.description,
            &list.scroll_info,
            &list.no_match,
        ]
        .into_iter()
        .map(|style| self.paint(style))
        .collect::<wasmtime::Result<Vec<_>>>()?;
        let options = (options.padding_x, options.autocomplete_max_visible);
        let operands = widget_editors::operands(&border, &list, keys, options);
        let kind = if keys.is_some() {
            "customEditor"
        } else {
            "editor"
        };
        let widget = self.build_widget(Build {
            kind,
            operands,
            ..Build::default()
        })?;
        let mut editor = Editor::new(border, options);
        editor.keys = keys;
        if let Ok(widget) = &widget {
            let key = self.table.get(widget)?.0;
            widget_editors::keep(&mut self.observed, key, editor);
        }
        Ok(widget)
    }

    /// The owned form of a wire editor request.
    pub(super) fn editor_request(&self, request: host::EditorRequest) -> wasmtime::Result<Request> {
        use host::EditorRequest as Wire;
        Ok(match request {
            Wire::SetBorderColor(color) => Request::SetBorderColor(self.paint(&color)?),
            Wire::GetText => Request::GetText,
            Wire::GetExpandedText => Request::GetExpandedText,
            Wire::GetLines => Request::GetLines,
            Wire::GetCursor => Request::GetCursor,
            Wire::AddToHistory(text) => Request::AddToHistory(text),
            Wire::GetPaddingX => Request::GetPaddingX,
            Wire::SetPaddingX(padding) => Request::SetPaddingX(padding),
            Wire::GetAutocompleteMaxVisible => Request::GetAutocompleteMaxVisible,
            Wire::SetAutocompleteMaxVisible(max) => Request::SetAutocompleteMaxVisible(max),
            Wire::GetDisableSubmit => Request::GetDisableSubmit,
            Wire::SetDisableSubmit(disabled) => Request::SetDisableSubmit(disabled),
            Wire::IsShowingAutocomplete => Request::IsShowingAutocomplete,
            Wire::GetFocused => Request::GetFocused,
            Wire::SetFocused(focused) => Request::SetFocused(focused),
        })
    }

    /// The route of a text change of the editor `this`.
    pub(super) fn edit_route(
        &mut self,
        this: &Resource<WidgetResource>,
        text: &str,
        insert: bool,
    ) -> wasmtime::Result<Resource<Route>> {
        let key = self.table.get(this)?.0;
        match widget_editors::edit(&mut self.observed, key, text, insert) {
            Ok(calls) => self.list_route(key, calls),
            Err(error) => self.routed(Err(error)),
        }
    }
}
