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

/// An editor factory another extension set, with whether it builds a custom editor.
pub struct ForeignFactory(pub bool);

impl host::HostEditorFactory for State {
    fn build(
        &mut self,
        this: Resource<ForeignFactory>,
        _: Resource<super::ui_host::Tui>,
        theme: host::EditorStyles,
        keybindings: Resource<super::widget_component::KeybindingsResource>,
    ) -> wasmtime::Result<Resource<Route>> {
        let custom = self.table.get(&this)?.0;
        let keys = custom.then(|| self.table.get(&keybindings).map(|keys| keys.0));
        let options = host::EditorOptions {
            padding_x: None,
            autocomplete_max_visible: None,
        };
        let built = match self.build_editor(&theme, keys.transpose()?, options)? {
            Ok(widget) => widget,
            Err(error) => return self.routed(Err(error)),
        };
        let key = self.table.get(&built)?.0;
        super::foreign_host::built(&mut self.observed, key, custom);
        let value = host::RouteValue::Editor(host::BuiltEditor {
            widget: built,
            custom,
        });
        self.answered(Ok(value))
    }
    fn drop(&mut self, this: Resource<ForeignFactory>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl State {
    /// A route that makes no nested call and answers `value`.
    fn answered(
        &mut self,
        value: Result<host::RouteValue, String>,
    ) -> wasmtime::Result<Resource<Route>> {
        self.push(Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(value),
        })
    }

    /// The owned form of a wire request on an editor another extension's factory built.
    fn foreign_op(
        &self,
        request: host::ForeignEditorRequest,
    ) -> wasmtime::Result<super::foreign_host::Op> {
        use super::foreign_host::Op;
        use host::ForeignEditorRequest as Wire;
        let handler = |slot, handler: Option<Resource<super::host::Identity>>| {
            let handler =
                handler.map(|handler| self.table.get(&handler).map(|identity| identity.0));
            Ok::<_, wasmtime::Error>(Op::SetHandler(
                widget_editors::slot_name(slot),
                handler.transpose()?,
            ))
        };
        Ok(match request {
            Wire::GetText => Op::GetText,
            Wire::SetText(text) => Op::Edit(text, false),
            Wire::InsertTextAtCursor(text) => Op::Edit(text, true),
            Wire::SetOnSubmit(callback) => handler(HandlerSlot::Submit, callback)?,
            Wire::SetOnChange(callback) => handler(HandlerSlot::Change, callback)?,
            Wire::OnChange => Op::OnChange,
            Wire::AddToHistory(text) => Op::Plain(Request::AddToHistory(text)),
            Wire::GetExpandedText => Op::Plain(Request::GetExpandedText),
            Wire::BorderColor => Op::BorderColor,
            Wire::SetBorderColor(style) => Op::SetBorderColor(self.paint(&style)?),
            Wire::SetPaddingX(padding) => Op::Plain(Request::SetPaddingX(padding)),
            Wire::SetAutocompleteMaxVisible(max) => {
                Op::Plain(Request::SetAutocompleteMaxVisible(max))
            }
            Wire::SetAutocompleteProvider(provider) => {
                Op::SetAutocompleteProvider(self.table.get(&provider)?.0)
            }
        })
    }

    /// The route of one request on an editor another extension's factory built.
    pub(super) fn foreign_route(
        &mut self,
        this: &Resource<WidgetResource>,
        request: host::ForeignEditorRequest,
    ) -> wasmtime::Result<Resource<Route>> {
        use super::foreign_host::Answer;
        use host::ForeignEditorAnswer as Wire;
        let key = self.table.get(this)?.0;
        let op = self.foreign_op(request)?;
        let answer = match super::foreign_host::request(&mut self.observed, key, op) {
            Ok(answer) => answer,
            Err(error) => return self.answered(Err(error)),
        };
        let answer = match answer {
            Answer::Done => Wire::Done,
            Answer::Absent => Wire::Absent,
            Answer::Text(text) => Wire::Text(text),
            Answer::Handler(lent) => Wire::Handler(
                lent.then(|| {
                    let name = super::foreign_host::LENT.to_owned();
                    self.push(super::widget_component::TextCallbackResource(name))
                })
                .transpose()?,
            ),
            Answer::Style(role) => {
                Wire::Style(self.push(super::widget_component::StyleResource(role))?)
            }
            Answer::Calls(calls) => {
                let route = self.list_route(key, calls)?;
                self.table.get_mut(&route)?.result =
                    Some(Ok(host::RouteValue::EditorAnswer(Wire::Done)));
                return Ok(route);
            }
        };
        self.answered(Ok(host::RouteValue::EditorAnswer(answer)))
    }
}
